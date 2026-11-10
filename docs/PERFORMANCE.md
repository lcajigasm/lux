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
| Open a 100 MB file — extra RSS | < 3× file size | **3.16×** (330.9 MB RSS / 104.9 MB file, `lux-core-cli` opening a real 100 MB file, `/usr/bin/time -l` `maximum resident set size`) | **fail at the time — fixed 2026-08-12, see below** |

The RSS overrun was a real, understood inefficiency, not measurement noise:
`Buffer::open` → `Buffer::from_bytes` → `encoding::decode` kept the
original `Vec<u8>` (from `fs::read`), a fresh owned `String` copy
(`str::from_utf8(bytes).to_owned()`), and `ropey`'s own internal chunk
storage (`Rope::from_str`) all alive at the same time at peak — three
full copies of a 100 MB file simultaneously, which is where the ~315 MB
peak RSS came from. The fix (reuse the `Vec<u8>` as the `String` via
`String::from_utf8` on the common all-UTF-8 fast path, avoiding one of the
three copies) was understood but not applied at Phase 1 close: widening
`Buffer::from_bytes`'s `&[u8]` signature to take ownership would have been
a wide-blast-radius change to the 250 existing call sites across the
crate that construct a `Buffer` from a borrowed byte literal, not
something to do under close-phase time pressure without its own
iteration. Left as an open item at the time; see the Phase 1 closing
report for the original tracking note.

**2026-08-12, revisited during a full-project review (not a new phase
close):** fixed without touching `from_bytes`'s signature at all — added
`encoding::decode_owned(bytes: Vec<u8>)`, a second entry point that takes
ownership so the plain-UTF-8-no-BOM case can do `String::from_utf8(bytes)`
(zero-copy reuse of the same allocation) instead of `str::from_utf8(&bytes).to_owned()`;
every other path (a BOM, invalid UTF-8) still copies, same as before.
`Buffer::open` (the only path this budget measures) now goes through a
new private `Buffer::from_owned_bytes` that calls `decode_owned`;
`Buffer::from_bytes(&[u8])` itself is untouched, so none of the 250
existing borrowing call sites needed to change. Cuts the three
simultaneous copies to two (the reused `Vec`/`String` allocation, plus
`ropey`'s own chunk storage). Re-measured the same way: **2.16×**
(215.6 MB RSS / 100.0 MB file, `lux-core-cli` opening a real 100 MB
file, `/usr/bin/time -l`) — **pass**, with headroom. `cargo test -p
lux-core buffer::` (35 tests, including a new
`encoding::decode_owned_agrees_with_decode_on_every_case` regression test
pinning `decode_owned` byte-for-byte identical to `decode` on every
encoding case) and the full workspace gate both clean.

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

**2026-10-31, v1.2.0, Linux x86_64 (cloud VM, not the MacBook Pro reference),
`--release`:**

| Metric | Budget | Measured | Result |
|---|---|---|---|
| `insert_at_cursor` plus syntax sync, 30 inserts in a 20,000-line Rust buffer (`test_continuous_typing_latency_large_file_under_8ms`) | < 8 ms p99 and max | **376 µs** p99 and max | pass |

The test's p99 index on 30 sorted samples is the slowest sample, so p99 and
max are the same number here. This is the edit-and-highlight path, not
keypress-to-pixel: there is still no GPU frame in the measurement. The
ongoing CI job is `release-latency` on `ubuntu-latest` only. On 2026-10-31,
before that cut, the same assert passed on hosted runners at 215 µs
(`ubuntu-latest`) and 1.06 ms (`windows-latest`). Debug `cargo test` keeps
a 120 ms ceiling. macos-15 debug run 41357599950 measured 82 ms while the
typing path was unchanged. The 8 ms release assert is not raised if a
hosted runner is slower; that result is written down instead.

**2026-11-02, file tree and tabs, Linux x86_64 cloud VM (Intel Xeon, 4 threads, not the MacBook Pro reference), `--release`.** Before is `main` `3661033`. After is the tree-and-tabs change. Typing is `test_continuous_typing_latency_large_file_under_8ms` (p99 on 30 samples is the slowest sample). Scroll is lux-render `shape_one_visible_window_of_50_never_before_seen_lines` (10 samples). Open is lux-benches `load_100mb` (10 samples). Scroll and the 100 MB load live in crates this change does not modify; the after column is a second sample of those same binaries. Typing is the new `lux` binary.

| Metric | Budget | Before | After |
|---|---|---|---|
| Typing, 30 inserts in a 20,000-line Rust buffer | < 8 ms p99 and max | 518 µs | 506 µs |
| Scroll, shape 50 new lines | not a release gate | 349 µs mean | 353 µs mean (no change, p = 0.74) |
| Open, load a 100 MB buffer | < 1 s | 242 ms mean | 241 ms mean |

The 8 ms release ceiling holds. 242 ms to load 100 MB is this VM. There is still no GPU frame in the typing or scroll sample.

## macos-15 runner, release Lux.app

`p0-macos.yml` measures the rows below on the free runner. Lux H1 is process start until the first presented frame, median of 10. H3 and H4 are a warm process opening the file on SIGUSR2 until that buffer's first presented frame. H2 is the key event until the next presented frame, p99, on a 20k-line file. H5 is the slowest redraw while a 50k-line file is scrolling. H6 is extra RSS across the 100 MB open. Sublime and Zed, when the free cask installs, are timed only until a window is listed. That clock is not H1. H2 and H5 are not readable from those processes. A missed budget stays in the table and does not fail the job.

The numbers are copied from the performance artifact after the run. This revision does not invent them.
