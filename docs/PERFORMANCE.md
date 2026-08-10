# Performance budgets

Measured, not aspirational (golden rule #3). "Feels fast" is not a
measurement. Reference hardware: MacBook Pro, M-series, `--release` build.
Windows/Linux thresholds get calibrated and recorded here once Phase 2 has a
shell to measure on those platforms.

| Metric | Budget |
|---|---|
| Cold start to first interactive frame | < 150 ms macOS, < 250 ms Windows/Linux |
| Keypress → updated pixel latency (p99) | < 8 ms |
| Frame during continuous scroll (50k-line file) | no frame > 16 ms; target 8.3 ms at 120 Hz |
| Open a 1 MB file to first render | < 50 ms |
| Open a 100 MB file | < 1 s; extra RSS < 3× file size |
| Insert 1 char at 1,000 cursors in a 100k-line file | < 5 ms |
| Tree-sitter reparse after a keystroke (10k lines) | < 2 ms |
| Goto Anything over 100,000 files | first results < 30 ms; full ranking < 150 ms |
| Find in files over 1 GB of code | < 2 s |
| Idle RSS with 10 tabs open | < 150 MB |
| Desktop binary size (release, compressed) | < 40 MB |
| Full `cargo test` run | < 90 s |

## Benchmark harness

`benches/` (crate `lux-benches`) holds the `criterion` benchmarks: the
Phase 0 smoke benchmark (`smoke_fibonacci_20`, still just pipeline-proving),
`phase1_budgets` (`insert_1_char_at_1000_cursors_100k_line_file`,
`buffer_load/load_100mb`) added when Phase 1's two named budgets became
measurable, and `phase3_budgets` (tree-sitter reparse after a keystroke,
Goto Anything's 100k-file ranking, and the reparse+highlight component for
the 16 ms frame budget — see the measurement history below) added at
Phase 3's close-out, and `phase4_budgets` (`find_in_files/search_1gb_corpus`,
a real on-disk 1 GB corpus) added with Phase 4's "Find in Files" deliverable.
Real budget benchmarks land in the phase that implements the capability
they measure (see the phase-by-phase acceptance criteria in
`LUX_PROMPT_MAESTRO.md`).

`crates/lux-render/benches/phase2_budgets.rs` holds Phase 2's three named
budgets (cold start, keypress → pixel, scroll), added as *component*
benchmarks — see the measurement history below for exactly what each one
measures and excludes, and why: no `wgpu` quad/text pipeline or unified
`lux` app exists yet to measure the full, literal metrics against. This
lives in `lux-render`'s own `benches/`, not `lux-benches`, because its gate
is `cargo bench -p lux-render`. Its `Cargo.toml` sets `[lib] bench = false`
so `cargo bench` doesn't also try (and fail) to run this crate's own
`#[cfg(test)]` unit tests through the incompatible built-in libtest bench
harness — `cargo test -p lux-render` is unaffected.

`criterion` measures wall-clock time only. The RSS half of the "open a
100 MB file" budget is measured separately with `/usr/bin/time -l` (macOS)
around `lux-core-cli` opening a real 100 MB file — see the entry below.

Save a baseline: `cargo bench -p lux-benches --bench <name> -- --save-baseline main`
Compare against it: `cargo bench -p lux-benches --bench <name> -- --baseline main`
Check the gate: `./scripts/check-bench-regression.sh main` (fails if any
benchmark's mean regressed more than 10% against the `main` baseline; see
`BENCH_REGRESSION_THRESHOLD` env var to override the percentage). It is kept
for controlled, repeatable measurements; CI does not run it while the only
benchmarks are this small, because hosted runners are too noisy.

## Measurement history

**2026-07-27, Phase 1 close, MacBook Pro M-series, `--release`:**

| Metric | Budget | Measured | Result |
|---|---|---|---|
| Insert 1 char at 1,000 cursors in a 100k-line file | < 5 ms | **0.97 ms** (mean, 100 samples) | pass |
| Open a 100 MB file — time | < 1 s | **85.4 ms** (mean, 10 samples) | pass |
| Open a 100 MB file — extra RSS | < 3× file size | **3.16×** (330.9 MB RSS / 104.9 MB file, `lux-core-cli` opening a real 100 MB file, `/usr/bin/time -l` `maximum resident set size`) | **fail** |

The RSS overrun is a real, understood inefficiency, not measurement noise:
`Buffer::open` → `Buffer::from_bytes` → `encoding::decode` keeps the
original `Vec<u8>` (from `fs::read`), a fresh owned `String` copy
(`str::from_utf8(bytes).to_owned()`), and `ropey`'s own internal chunk
storage (`Rope::from_str`) all alive at the same time at peak — three
full copies of a 100 MB file simultaneously, which is where the ~315 MB
peak RSS comes from. The fix (reuse the `Vec<u8>` as the `String` via
`String::from_utf8` on the common all-UTF-8 fast path, avoiding one of the
three copies) is understood but not applied: `Buffer::from_bytes` takes
`&[u8]`, a signature hundreds of existing tests across the crate call with
borrowed literals (`b"..."`), and widening it to take ownership would be a
wide-blast-radius change to already-closed, heavily-tested modules — not
something to do under close-phase time pressure without its own
iteration. Left as an open item; see the Phase 1 closing report for
tracking.

**2026-07-30, Phase 2 deliverable "Presupuestos de rendimiento", MacBook
Pro M-series, `--release` (`cargo bench -p lux-render`):**

| Metric (component measured) | Budget (full metric) | Measured | Result |
|---|---|---|---|
| `cold_start/gpu_context_new` — adapter+device negotiation only | < 150 ms (full cold start) | **0.61 ms** (mean, 20 samples) | component passes; full metric **not measurable yet** |
| `keypress_reshape_and_rasterize_new_glyph` — reshape edited line + rasterize one new glyph | < 8 ms p99 (full keypress→pixel) | **6.4 µs** (mean, 100 samples) | component passes; full metric **not measurable yet** |
| `scroll/shape_one_visible_window_of_50_never_before_seen_lines` — shape 50 fresh lines | no frame > 16 ms (full scroll frame) | **0.21 ms** (mean, 100 samples) | component passes; full metric **not measurable yet** |

None of these three rows is the literal budget from `LUX_PROMPT_MAESTRO.md`
— each measures only the CPU-bound piece of it that exists in `lux-render`
today (see `phase2_budgets.rs`'s own doc comments for exactly what's
excluded from each one: window creation and first paint for cold start;
input-event dispatch and GPU submit/present for keypress latency;
`lux-ui`'s per-frame geometry work and the actual draw for scroll). All
three components measure comfortably within their share of the full
budget, which is a good sign, not a substitute for measuring the real
thing once the `wgpu` quad/text pipeline and a unified `lux` binary exist.
This is not a regression-gated baseline: `cargo bench -p lux-render --
--baseline none` (the gate this deliverable specifies) compares against a
saved baseline literally named `none`, created once with `--save-baseline
none` before this table's numbers were captured — re-run `--save-baseline
none` to refresh it if these benchmarks change in a way where you want a
new zero-point.

**2026-08-10, Phase 3 close-out, MacBook Pro M-series, `--release`
(`cargo bench -p lux-benches --bench phase3_budgets`):**

| Metric | Budget | Measured | Result |
|---|---|---|---|
| Tree-sitter reparse after a keystroke (10k lines) | < 2 ms | **1.05 ms** (mean, 100 samples) | pass |
| Goto Anything fuzzy ranking over 100,000 files | first results < 30 ms; full ranking < 150 ms | **1.34 ms** (mean, 10 samples) | pass — see note below on "first results" |
| Reparse + highlight after a keystroke, **whole 20k-line document** | no frame > 16 ms | **16.77 ms** (mean, 100 samples) | **fails** — kept intentionally, see note |
| Reparse + highlight after a keystroke, **visible 50-line range** | no frame > 16 ms | **2.30 ms** (mean, 100 samples) | pass |

Three real findings came out of writing these, not just numbers to fill
in a table:

1. **`Document::highlight` scanned the whole tree on every call**, not
   just the edited or visible range — fine for one-shot uses (golden
   token tests, opening a file) but the wrong thing to call after every
   keystroke on a large file, which is exactly what this budget measures.
   Added `Document::highlight_range(source, byte_range)`
   (`tree_sitter::QueryCursor::set_byte_range`, not a post-hoc filter) so
   a UI can re-highlight only what's on screen — the row above simulates
   scoping the query to a 50-line window around the edit, the way
   `EditorView` already scopes glyph shaping to `visible_line_range`. The
   whole-document row is kept in the benchmark suite on purpose, as a
   standing demonstration of why scoping is required, not an alternative
   path to the budget.
2. **`Query::new` — compiling the highlight pattern set — was itself the
   dominant cost**, not the tree traversal: it was being called fresh on
   every `highlight`/`highlight_range` call, and a language's
   `highlights.scm` is hundreds of lines. Caching the compiled `Query` as
   a `Document` field (built once in `Document::new`) cut the
   whole-document number from ~26 ms to ~16.8 ms *and* the visible-range
   number from ~11.6 ms to ~2.3 ms — the range-scoping fix and the
   query-caching fix compound, and neither alone would have hit budget
   with real margin.
3. **Goto Anything's "first results < 30 ms" isn't a separately
   measurable quantity for this implementation.** `lux-search::fuzzy::
   rank_paths` is `nucleo`'s synchronous `Pattern::match_list` (see ADR
   0009 and the Goto Anything receipt) — one call produces the complete,
   sorted result set, so there's no partial/incremental "first results"
   phase to time separately the way there would be for the async
   `Nucleo<T>` picker. The 1.34 ms measured here is compared against the
   stricter 150 ms full-ranking budget and clears it with over 100×
   margin; migrating to `Nucleo<T>` for a genuine incremental result
   stream is future work, only worth doing once a real 100k-file
   workspace index exists (Phase 4) to make it matter.

See `benches/benches/phase3_budgets.rs` for the exact scenarios (source
generation, edit-site selection, visible-range simulation) and
`crates/lux-syntax/src/lib.rs`'s `highlight`/`highlight_range` doc
comments for the API-level explanation.

**2026-08-10, Phase 4 "Find in Files" deliverable, MacBook Pro M-series,
`--release` (`cargo bench -p lux-benches --bench phase4_budgets --
--save-baseline none`):**

| Metric | Budget | Measured | Result |
|---|---|---|---|
| Find in files over 1 GB of code | < 2 s | **43.6 ms** (mean, 10 samples) | pass, ~46× margin |

Real, on-disk corpus, not a synthetic timing estimate: 2,000 files across
20 subdirectories, ~1.0722 GB total (`benches/benches/phase4_budgets.rs`'s
`make_1gb_corpus`, generated fresh before the benchmark and removed after —
see that file's own doc comment), searched end-to-end with
`lux_search::find_in_files` for a literal needle placed on every 500th
line (30,000 real matches collected via `SearchStream::wait_until_done`,
not a dry run that stops at the first hit). The wide margin comes from two
real properties of the implementation, not benchmark looseness: `ignore`'s
`WalkParallel` fans the 2,000 files out across all available CPU cores
(this reference hardware has far more than the single-threaded case would
imply), and criterion's repeated sampling means every sample after the
first benefits from the OS page cache already holding the corpus in
memory — representative of a warm repeated search during a real working
session, less so of a genuinely cold first search after a reboot. Neither
caveat is unique to this benchmark (the 100 MB buffer-load benchmark above
has the same page-cache dynamic); noted here because 46× margin is large
enough that it's worth being explicit about what "43.6 ms" does and
doesn't represent, rather than letting the number imply more precision
than it has.
