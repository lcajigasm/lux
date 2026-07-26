---
description: Run a crate's full verification gate and report pass/fail without softening it.
---
For crate $ARGUMENTS, run in order:

1. `cargo check -p $ARGUMENTS`
2. `cargo clippy -p $ARGUMENTS -- -D warnings`
3. `cargo test -p $ARGUMENTS`
4. `cargo bench -p lux-benches -- --baseline main` (only if $ARGUMENTS'
   deliverable has a real budget benchmark yet — see `docs/PERFORMANCE.md`;
   skip otherwise, benchmarks live in the `lux-benches` crate, not per-crate)
5. `./scripts/check-limits.sh`

Report the result of every step. If anything fails, stop right there and show
the exact failure output — do not summarize or soften it. Do not declare
success unless all applicable steps actually passed.

This command is the independent check called for in
`LUX_LOOP_ENGINEERING.md` section 3: the generator that wrote the code must
not be the one judging whether it's done. Run this even after a loop
(`/goal` or `scripts/verify-loop.sh`) already reported success.
