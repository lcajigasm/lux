# Receipt: <deliverable name>

A receipt records one loop's execution — how many iterations it took, what
failed along the way, what fixed it, and whether it got stuck. It is not an
ADR: an ADR explains a design decision that survives forever, a receipt is
the log of one concrete run, mainly so the next session knows exactly where
the work was left without you having to explain it.

See `LUX_LOOP_ENGINEERING.md` sections 4-5 for the full method this supports.

- Phase: <NN>
- Crate(s): <crate>
- Goal: <one sentence, verifiable condition — not a task list>
- Verify: `<exact command that decides pass/fail>`
- Iterations: <n> / <max>
- Result: passed | stuck | budget-exhausted
- Opened: <date>
- Closed: <date>
- Notes: what failed along the way, what the fix was, and whether any of it
  is worth promoting to `CLAUDE.md` as a durable correction (see the loop
  engineering doc's "four layers of memory").
