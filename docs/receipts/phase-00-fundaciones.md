# Receipt: phase-00-fundaciones

- Phase: 00
- Crate(s): workspace-wide (no domain crate has real code yet, by design)
- Goal: the scaffolding that prevents v0's failures exists before any code
  that could repeat them — workspace, CLAUDE.md, ADRs 0001-0012, re-verified
  dependency versions, full CI, size-limit and core-isolation gates, a
  criterion benchmark harness with baseline storage, `docs/STATUS.md`,
  `deny.toml`, and the loop-engineering operational layer.
- Verify: `cargo build --workspace`, `./scripts/check-limits.sh`,
  `./scripts/check-core-isolation.sh`, `cargo bench -p lux-benches -- --save-baseline main`
- Iterations: spans two sessions (most of Phase 0 landed before this one;
  this session verified it, fixed one bug, and added the loop-engineering
  layer)
- Result: passed
- Opened: 2026-07-25 (per `docs/DEPENDENCIES.md` original research date)
- Closed: 2026-07-26

## What was already in place at the start of this session

Commits `253c5c2` through `ece3685` had already built the substance of
Phase 0: the Cargo workspace with all crates scaffolded as empty stubs,
`CLAUDE.md` (verbatim from the master prompt's Annex A), ADRs 0001-0012,
`docs/DEPENDENCIES.md` with versions re-checked against crates.io on
2026-07-26, `docs/STATUS.md` with the full not-started capability matrix,
`deny.toml`, `scripts/check-limits.sh`, `scripts/check-core-isolation.sh`,
`scripts/check-bench-regression.sh`, the `lux-benches` criterion harness, the
`xtask verify` task, and a GitHub Actions CI workflow with all required jobs.

## What this session did

1. **Audited all of the above against the master prompt's Phase 0 acceptance
   criteria** rather than assuming the prior session's commits were
   sufficient — read every script, the CI workflow, `deny.toml`, and
   `docs/DEPENDENCIES.md` in full.
2. **Found and fixed a real bug**: `cargo bench --workspace -- --save-baseline main`
   (used in both the CI `bench` job and implicitly expected by the acceptance
   criterion "benchmarks run and save a baseline") fails. Every crate's
   lib/bin target gets an implicit libtest bench harness by default under
   `--workspace`, and that harness doesn't understand criterion's
   `--save-baseline`/`--baseline` flags, so the job aborts on the first
   non-`lux-benches` crate it reaches. Fixed by scoping the CI bench job to
   `-p lux-benches` and dropping `lux-benches`'s own unused empty `[lib]`
   target (which had the identical problem). Verified the scoped command
   works end to end (bootstraps a baseline, runs the smoke benchmark).
   Committed separately (`8c228f4`) before touching anything else, since it's
   a correctness fix independent of the loop-engineering work.
3. **Verified, live, the two acceptance criteria that require a deliberately
   introduced violation** rather than trusting that a prior session's tool
   permissions history meant they still worked:
   - Added a 601-line file → `check-limits.sh` correctly failed (exit 1) and
     named the exact file and line count. Removed it afterward.
   - Added `wgpu` as a direct dependency of `lux-core` → `check-core-isolation.sh`
     correctly failed (exit 1) and printed the dependency chain. Reverted
     `Cargo.toml`/`Cargo.lock` afterward via `git checkout`.
   - Re-ran `cargo build --workspace`, `check-limits.sh`, and
     `check-core-isolation.sh` clean after reverting, to confirm no residue.
4. **Confirmed `CLAUDE.md` matches Annex A verbatim** — no changes needed.
5. **Built the loop-engineering infrastructure** from `LUX_LOOP_ENGINEERING.md`,
   none of which existed yet:
   - `scripts/verify-loop.sh` — existing draft, fixed two Spanish field names
     (`Resultado`/`Cerrado` → `Result`/`Closed`) left over from translation,
     for consistency with the English-only repo convention; made it
     executable.
   - `.claude/hooks/stop-verify.sh` + `.claude/settings.json` — a Stop hook
     that blocks ending a session while `cargo test -p $LUX_ACTIVE_CRATE` is
     red. This is the technical enforcement behind golden rule #1 ("nothing
     simulated"); CLAUDE.md alone is context, not enforcement. Verified all
     three branches live: no-op when `LUX_ACTIVE_CRATE` is unset (exit 0),
     passes silently when the crate's tests pass (exit 0), and blocks with a
     clear message when they don't (exit 2).
   - `docs/receipts/TEMPLATE.md` — English translation of the template in
     `LUX_LOOP_ENGINEERING.md` section 5.
   - `.claude/commands/verify-crate.md`, `close-phase.md`, `write-adr.md` —
     adapted from the tutorial's section 8, with one change: `close-phase`
     doesn't hard-require `LUX_PROMPT_MAESTRO.md` to exist in the repo (see
     the decision below), it falls back to asking the user for the phase's
     acceptance criteria if the file isn't present.
   - `.claude/skills/{add-tree-sitter-grammar,add-platform-backend,benchmark-against-budget}/SKILL.md` —
     written deliberately thin per the tutorial's own advice ("write these in
     Phase 0 even nearly empty, fill them in the first time you actually need
     them"), but each already encodes the one gotcha known today (grammar
     version conflicts, iOS's no-child-process constraint, the
     `--workspace` bench bug from item 2).

## Decisions made along the way (flagging per your instruction to stop and
ask rather than improvise silently on anything uncertain)

- **`LUX_PROMPT_MAESTRO.md` and `LUX_LOOP_ENGINEERING.md` are gitignored, not
  committed.** Their own preambles say they're working documents for this
  human/AI collaboration ("está en español para ti... todo el artefacto del
  repositorio... va en inglés" — the master prompt explicitly excludes
  itself from "repository artifact"). Committing them in Spanish would
  violate the English-only convention (`CLAUDE.md` / working agreement
  section "Conventions") for no benefit, since their operative content is
  already extracted into `CLAUDE.md`, the 12 ADRs, and this loop-engineering
  setup. I judged this unambiguous enough not to interrupt you for it, but
  it's easy to reverse — say the word and I'll commit them (translated, if
  you want them as first-class repo docs) or move them under a clearly
  labeled `notes/` directory instead.
- **The bench `--workspace` bug (item 2 above) was fixed, not just
  reported**, because it's a straightforward tooling correctness issue with
  one unambiguous fix, not an architectural fork in the road — it doesn't
  fall under "when a target is unreachable, stop and report" (golden rule
  #10), which is about design decisions, not broken glue code.
- **Did not open a real PR to verify "CI verde en un PR de prueba"** (one of
  Phase 0's acceptance criteria) — that requires pushing to GitHub, which I
  won't do without being asked. The workflow file has been read in full and
  every job's command has been run locally with the same invocations CI
  uses; I'm confident in the config but flagging that the literal
  acceptance criterion (a GitHub Actions run going green) is still
  unverified on GitHub's infrastructure itself.

## Notes for future sessions

- `cargo bench --workspace` is now a known trap — documented in this
  receipt and in the `benchmark-against-budget` skill so it doesn't get
  reintroduced by muscle memory in a later phase's CI changes.
- The next real use of `/close-phase` will be for Phase 1; at that point,
  decide whether `LUX_PROMPT_MAESTRO.md` should be pasted fresh into that
  session (per the tutorial's own recommended workflow: paste one phase's
  section per session) or whether the untracked file happens to still be on
  disk. Don't assume the latter across machines or fresh clones.
