---
name: benchmark-against-budget
description: Run a criterion benchmark against the stored baseline, tell a real regression from machine noise, and update docs/PERFORMANCE.md with the result.
---

# Benchmark against the performance budget

How to check a change against the numbers in `docs/PERFORMANCE.md` (golden
rule #3: performance budgets are tests, not aspirations). This is the most
frequently used skill in the project — nearly every phase touches a budget.

1. **Never use `cargo bench --workspace`.** Every crate's lib/bin target gets
   an implicit libtest bench harness by default, and that harness doesn't
   understand criterion's `--save-baseline`/`--baseline` flags — the job
   aborts. Scope to the crate that actually has `[[bench]]` targets:
   `cargo bench -p lux-benches -- --baseline main` (see the fix in
   `docs/receipts/phase-00-fundaciones.md` for how this bit CI once already).
2. **Save a baseline** (first run, or deliberately resetting the "new
   normal" after a reviewed perf-affecting change):
   `cargo bench -p lux-benches --bench <name> -- --save-baseline main`
3. **Compare against it:**
   `cargo bench -p lux-benches --bench <name> -- --baseline main`
4. **Check the gate:** `./scripts/check-bench-regression.sh main` — fails if
   any benchmark's mean regressed more than 10% (`BENCH_REGRESSION_THRESHOLD`
   env var to override). Run it only on repeatable hardware.
5. **Telling signal from noise:** criterion's own report already flags
   outliers and gives confidence intervals — read those before trusting a
   single percentage. Re-run once if a regression is borderline (a few
   percent over threshold) before concluding it's real; a >20% swing on a
   quiet machine is real, a 11% swing next to a wide confidence interval
   often isn't. When genuinely unsure, say so rather than guessing — this is
   exactly the kind of measurement golden rule #3 says "feels fast" cannot
   substitute for.
6. **Update `docs/PERFORMANCE.md`** — under "Measurement history" — with the
   number, the date, and what changed, once a real capability (not the
   `smoke` placeholder) has a working budget benchmark.

Do not gate benchmarks on GitHub-hosted runners: their VM and cache placement
vary too much. Add a gate when a repeatable runner and a real budget benchmark
exist.
