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
and `phase1_budgets` (`insert_1_char_at_1000_cursors_100k_line_file`,
`buffer_load/load_100mb`) added when Phase 1's two named budgets became
measurable. Real budget benchmarks land in the phase that implements the
capability they measure (see the phase-by-phase acceptance criteria in
`LUX_PROMPT_MAESTRO.md`).

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
