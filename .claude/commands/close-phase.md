---
description: Close a Lux project phase — verify deliverables, update STATUS.md, prepare the release tag.
---
Phase to close: $ARGUMENTS

1. Get that phase's acceptance criteria. If `LUX_PROMPT_MAESTRO.md` exists at
   the repo root, read the criteria from there. If it doesn't (it's a working
   document for this human/AI collaboration, not a committed repo artifact —
   see its own preamble), ask the user to paste the phase's acceptance
   criteria before continuing.
2. For each deliverable, check that its receipt exists under `docs/receipts/`
   and says `Result: passed`. If any is missing or not passed, stop here and
   list exactly what's outstanding — do not continue.
3. Run the full workspace gate:
   `cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo deny check && ./scripts/check-limits.sh && ./scripts/check-core-isolation.sh`
4. Update `docs/STATUS.md` with the newly landed capabilities, per platform.
5. Draft release notes from the commits since the last tag.
6. Propose the version tag for this phase. Wait for confirmation before
   pushing it.
