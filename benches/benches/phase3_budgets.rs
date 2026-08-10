//! Phase 3's named performance budgets (`LUX_PROMPT_MAESTRO.md`): tree-sitter
//! reparse after a keystroke on a 10k-line file (< 2 ms), and Goto Anything's
//! fuzzy ranking over 100,000 files (first results < 30 ms; full ranking
//! < 150 ms). A third function measures reparse + highlight together on a
//! 20k-line file, the CPU-bound component of "no frame > 16 ms while typing
//! with highlighting active" — see its own doc comment for exactly what
//! that excludes and why the full metric isn't measurable yet, the same
//! "component, not the literal budget" approach `lux-render`'s
//! `phase2_budgets.rs` already uses and `docs/PERFORMANCE.md` documents.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use lux_syntax::{Document, LanguageId};
use std::hint::black_box;
use tree_sitter::{InputEdit, Point};

const RUST_LINE_COUNT: usize = 10_000;
const RUST_LINE_COUNT_20K: usize = 20_000;

/// `count` lines of real, parseable Rust: `count / 4` small functions, each
/// with a numeric literal a "keystroke" can land inside.
fn make_rust_source(count: usize) -> String {
    let functions = count / 4;
    let mut source = String::with_capacity(count * 24);
    for i in 0..functions {
        source.push_str(&format!(
            "fn f{i}() -> i32 {{\n    let x = {i};\n    x + 1\n}}\n"
        ));
    }
    source
}

/// The byte offset of the last digit of the numeric literal in the middle
/// function's `let x = <N>;` line — a realistic "typing inside an
/// identifier/literal" edit site, not an arbitrary offset.
fn middle_edit_offset(source: &str, count: usize) -> usize {
    let middle_fn = format!("fn f{}()", count / 4 / 2);
    let fn_start = source
        .find(&middle_fn)
        .expect("generated function must exist");
    let let_start = source[fn_start..].find("let x = ").unwrap() + fn_start;
    source[let_start..].find(';').unwrap() + let_start
}

/// The `tree_sitter::Point` (row, column) of `byte_offset` in `text`.
/// Byte-based, matching `Point::column`'s own units — fine here since the
/// generated source is pure ASCII, so byte and char offsets coincide.
fn point_at(text: &str, byte_offset: usize) -> Point {
    let prefix = &text[..byte_offset];
    let row = prefix.matches('\n').count();
    let column = match prefix.rfind('\n') {
        Some(newline) => byte_offset - newline - 1,
        None => byte_offset,
    };
    Point { row, column }
}

/// A single-character insertion at `offset`, as the `InputEdit` +
/// post-edit source `Document::edit` needs.
fn keystroke_at(source: &str, offset: usize) -> (InputEdit, String) {
    let start = point_at(source, offset);
    let mut new_source = source.to_string();
    new_source.insert(offset, '0');
    let edit = InputEdit {
        start_byte: offset,
        old_end_byte: offset,
        new_end_byte: offset + 1,
        start_position: start,
        old_end_position: start,
        new_end_position: Point {
            row: start.row,
            column: start.column + 1,
        },
    };
    (edit, new_source)
}

/// `LUX_PROMPT_MAESTRO.md`: "Reparseo tras pulsación en 10k líneas < 2 ms".
/// Measures `Document::edit` alone — the incremental reparse a keystroke
/// triggers — not the initial full parse (done once, in `iter_batched`'s
/// untimed setup) and not highlighting (a separate function below).
fn bench_reparse_after_keystroke_10k_lines(c: &mut Criterion) {
    let source = make_rust_source(RUST_LINE_COUNT);
    let offset = middle_edit_offset(&source, RUST_LINE_COUNT);

    c.bench_function("treesitter_reparse_after_keystroke_10k_lines", |b| {
        b.iter_batched(
            || {
                let mut doc = Document::new(LanguageId::Rust);
                doc.set_text(&source);
                let (edit, new_source) = keystroke_at(&source, offset);
                (doc, edit, new_source)
            },
            |(mut doc, edit, new_source)| {
                doc.edit(black_box(&edit), black_box(&new_source));
                black_box(&doc);
            },
            BatchSize::LargeInput,
        );
    });
}

/// The byte range of a `radius_lines`-above/below window around `offset`'s
/// line — a stand-in for "the editor's visible range", the same shape
/// `EditorView::build_frame`'s `visible_line_range` computes from a real
/// viewport (Phase 2 deliverable 4), just line-counted here since there's
/// no `Viewport`/font metrics in this benchmark.
fn visible_range_around(
    source: &str,
    offset: usize,
    radius_lines: usize,
) -> std::ops::Range<usize> {
    let newlines: Vec<usize> = source.match_indices('\n').map(|(i, _)| i).collect();
    let row = point_at(source, offset).row;
    let start_row = row.saturating_sub(radius_lines);
    let end_row = (row + radius_lines).min(newlines.len());
    let start = if start_row == 0 {
        0
    } else {
        newlines[start_row - 1] + 1
    };
    let end = newlines.get(end_row).copied().unwrap_or(source.len());
    start..end
}

/// Component measurement for "Ningún frame > 16 ms mientras se teclea en un
/// fichero de 20k líneas con resaltado activo" — the *naive* way, calling
/// `Document::highlight`, which runs its query over the **entire** tree on
/// every call (`cursor.captures(&query, tree.root_node(), ...)`, not scoped
/// to the edited or visible range). Unlike the pure reparse above, this
/// isn't incremental, and it's what motivated `Document::highlight_range`
/// (see that method's own doc comment, which cites this benchmark's
/// measured ~16.8 ms — right at/over the 16 ms budget, even after also
/// caching the compiled `Query` per `Document` instead of recompiling it
/// every call, which is what brought this down from an initial ~26 ms).
/// Kept as a regression/motivation marker, not the number a real UI should
/// hit — see `bench_reparse_and_highlight_visible_range_after_keystroke_20k_lines`
/// below for that one.
fn bench_reparse_and_highlight_full_document_after_keystroke_20k_lines(c: &mut Criterion) {
    let source = make_rust_source(RUST_LINE_COUNT_20K);
    let offset = middle_edit_offset(&source, RUST_LINE_COUNT_20K);

    c.bench_function(
        "reparse_and_highlight_full_document_after_keystroke_20k_lines",
        |b| {
            b.iter_batched(
                || {
                    let mut doc = Document::new(LanguageId::Rust);
                    doc.set_text(&source);
                    let (edit, new_source) = keystroke_at(&source, offset);
                    (doc, edit, new_source)
                },
                |(mut doc, edit, new_source)| {
                    doc.edit(black_box(&edit), black_box(&new_source));
                    black_box(doc.highlight(black_box(&new_source)));
                },
                BatchSize::LargeInput,
            );
        },
    );
}

/// The same scenario as above, but calling `Document::highlight_range`
/// scoped to a 50-line window around the edit (25 lines above/below) —
/// what a real UI actually needs after a keystroke: re-highlight what's on
/// screen, not the whole file. This is the number that should meet the
/// 16 ms budget; the whole-document version above exists to show why
/// scoping is required, not as an alternative way to hit it.
fn bench_reparse_and_highlight_visible_range_after_keystroke_20k_lines(c: &mut Criterion) {
    let source = make_rust_source(RUST_LINE_COUNT_20K);
    let offset = middle_edit_offset(&source, RUST_LINE_COUNT_20K);

    c.bench_function(
        "reparse_and_highlight_visible_range_after_keystroke_20k_lines",
        |b| {
            b.iter_batched(
                || {
                    let mut doc = Document::new(LanguageId::Rust);
                    doc.set_text(&source);
                    let (edit, new_source) = keystroke_at(&source, offset);
                    let visible = visible_range_around(&new_source, offset, 25);
                    (doc, edit, new_source, visible)
                },
                |(mut doc, edit, new_source, visible)| {
                    doc.edit(black_box(&edit), black_box(&new_source));
                    black_box(doc.highlight_range(black_box(&new_source), visible));
                },
                BatchSize::LargeInput,
            );
        },
    );
}

fn make_100k_paths() -> Vec<String> {
    (0..100_000)
        .map(|i| {
            format!(
                "src/module_{:04}/component_{:05}/file_{i:05}.rs",
                i / 2500,
                i / 50
            )
        })
        .collect()
}

/// `LUX_PROMPT_MAESTRO.md`: "Goto Anything sobre 100.000 ficheros: primeros
/// resultados < 30 ms; ranking completo < 150 ms". `lux-search::fuzzy::
/// rank_paths` (see `docs/adr/0009-search-and-fuzzy-matching.md` and
/// `docs/receipts/phase-03-goto-anything-nucleo-sufijos-ficherolinea.md`)
/// is the synchronous, single-threaded `nucleo::Pattern::match_list` —
/// deliberately chosen over the async, incrementally-streaming `Nucleo<T>`
/// picker because there's no real workspace file index yet to page results
/// from (Phase 4, not built). That also means "first results" and "full
/// ranking" aren't two separately-measurable phases for this
/// implementation the way they would be for `Nucleo<T>`'s snapshot-based
/// API — one synchronous call produces both at once, so this measures
/// against the stricter 150 ms full-ranking budget; migrating to `Nucleo<T>`
/// for a genuinely incremental "first results" number is future work if a
/// real 100k-file index ever makes that worth doing (the same migration
/// path the ADR already names).
fn bench_goto_anything_100k_files(c: &mut Criterion) {
    let paths = make_100k_paths();
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();

    let mut group = c.benchmark_group("goto_anything");
    // A 100k-item fuzzy match per sample isn't free; 10 samples is still
    // enough for a stable mean at this scale, same reasoning as
    // `phase1_budgets.rs`'s `buffer_load/load_100mb` group.
    group.sample_size(10);
    group.bench_function("rank_paths_100k_files", |b| {
        b.iter(|| {
            let results =
                lux_search::fuzzy::rank_paths(black_box("file00050"), path_refs.iter().copied());
            black_box(results);
        });
    });
    group.finish();
}

criterion_group!(
    budgets,
    bench_reparse_after_keystroke_10k_lines,
    bench_reparse_and_highlight_full_document_after_keystroke_20k_lines,
    bench_reparse_and_highlight_visible_range_after_keystroke_20k_lines,
    bench_goto_anything_100k_files
);
criterion_main!(budgets);
