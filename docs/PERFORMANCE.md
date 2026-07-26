# Performance budgets

Measured, not aspirational (golden rule #3). A regression greater than 10%
against the stored `criterion` baseline fails CI. "Feels fast" is not a
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
| Tree-sitter reparse after a keystroke (10k lines) | < 2 ms |
| Goto Anything over 100,000 files | first results < 30 ms; full ranking < 150 ms |
| Find in files over 1 GB of code | < 2 s |
| Idle RSS with 10 tabs open | < 150 MB |
| Desktop binary size (release, compressed) | < 40 MB |
| Full `cargo test` run | < 90 s |

## Benchmark harness

`benches/` (crate `lux-benches`) holds the `criterion` benchmarks. As of Phase
0 it contains one smoke benchmark (`smoke_fibonacci_20`) that exists only to
prove the pipeline — run, save baseline, compare — works end to end; it does
not measure anything in the table above, because nothing above has a working
implementation yet. Real budget benchmarks land in the phase that implements
the capability they measure (see the phase-by-phase acceptance criteria in
`LUX_PROMPT_MAESTRO.md`).

Save a baseline: `cargo bench -p lux-benches --bench <name> -- --save-baseline main`
Compare against it: `cargo bench -p lux-benches --bench <name> -- --baseline main`
Check the gate: `./scripts/check-bench-regression.sh main` (fails if any
benchmark's mean regressed more than 10% against the `main` baseline; see
`BENCH_REGRESSION_THRESHOLD` env var to override the percentage).

CI (`.github/workflows/ci.yml`, job `bench`) stores the baseline as a GitHub
Actions cache under the fixed key `criterion-baseline-main`. Cache entries
are immutable once created, so the baseline does **not** silently drift with
every green `main` build — it only gets established once and then every run
compares against that same fixed point. To deliberately accept a new normal
(after a reviewed, intentional performance-affecting change), delete the
cache entry (`gh cache delete criterion-baseline-main` or via the repo's
Actions → Caches UI) so the next run on `main` re-bootstraps it.

## Measurement history

_No real budget has been measured yet. Phase 0 ships no UI and no engine._
