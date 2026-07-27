//! The two performance budgets Phase 1's acceptance criteria name
//! explicitly (LUX_PROMPT_MAESTRO.md): inserting one char at 1,000 cursors
//! in a 100k-line file (< 5 ms), and loading a 100 MB file (< 1 s; the RSS
//! part of that budget isn't something `criterion` measures — see
//! `docs/PERFORMANCE.md`'s measurement history for that half, taken with
//! `/usr/bin/time -l`).

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use lux_core::buffer::Buffer;
use lux_core::edit_ops::{apply_to_each, insert_text};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use std::hint::black_box;

const LINE_COUNT: usize = 100_000;
const CURSOR_COUNT: usize = 1_000;

/// Fixed-width lines so a cursor's char offset for line `n` is just `n *
/// line_len` — no need to reach into `lux-core`'s internals (this crate
/// only sees its public API) to look up line starts.
fn make_100k_line_text() -> (String, usize) {
    let line = |i: usize| format!("line {i:06} of the file with some more padding text\n");
    let line_len = line(0).chars().count();
    let mut text = String::with_capacity(LINE_COUNT * line_len);
    for i in 0..LINE_COUNT {
        text.push_str(&line(i));
    }
    (text, line_len)
}

fn bench_multicursor_insert(c: &mut Criterion) {
    let (text, line_len) = make_100k_line_text();
    let cursor_positions: Vec<usize> = (0..CURSOR_COUNT).map(|n| n * line_len).collect();

    c.bench_function("insert_1_char_at_1000_cursors_100k_line_file", |b| {
        b.iter_batched(
            || {
                let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
                let cursors: Vec<Selection> = cursor_positions
                    .iter()
                    .map(|&idx| Selection::cursor(Position::from_char(&buffer, idx)))
                    .collect();
                (buffer, MultiCursor::from_cursors(cursors))
            },
            |(mut buffer, mc)| {
                let result = apply_to_each(&mut buffer, &mc, |buffer, sel| {
                    insert_text(buffer, sel, "x")
                });
                black_box(result);
            },
            BatchSize::LargeInput,
        );
    });
}

fn make_100mb_text() -> Vec<u8> {
    let line = "the quick brown fox jumps over the lazy dog, again and again\n";
    let target_bytes = 100 * 1024 * 1024;
    let repeats = target_bytes / line.len();
    line.repeat(repeats).into_bytes()
}

fn bench_buffer_load_100mb(c: &mut Criterion) {
    let bytes = make_100mb_text();
    let mut group = c.benchmark_group("buffer_load");
    // A 100 MB parse is slow enough that criterion's default 100 samples
    // would take several minutes; 10 is still enough for a stable mean at
    // this scale.
    group.sample_size(10);
    group.bench_function("load_100mb", |b| {
        b.iter(|| {
            let buffer = Buffer::from_bytes(black_box(&bytes)).unwrap();
            black_box(buffer);
        });
    });
    group.finish();
}

criterion_group!(budgets, bench_multicursor_insert, bench_buffer_load_100mb);
criterion_main!(budgets);
