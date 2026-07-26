---
description: Create a new numbered ADR in docs/adr/ following the project's format.
---
Topic: $ARGUMENTS

1. Find the next ADR number: check the highest-numbered file in `docs/adr/`
   and increment it (zero-padded to 4 digits, e.g. `0013`).
2. Ask the user for whatever isn't already clear from context: the decision
   title, the alternatives considered, and the consequences (including
   downsides accepted, not just upsides).
3. Write `docs/adr/NNNN-kebab-case-title.md` following the structure already
   used by `docs/adr/0001-lessons-from-v0.md`: `# NNNN. Title`, then
   `## Status` (Accepted, unless the user says otherwise), `## Context`,
   `## Decision`, `## Consequences`.
4. If this ADR reverses or narrows a previous one, say so explicitly in
   `## Context` and reference the old ADR by number — never edit the old
   ADR's text (golden rule #10: to reverse a decision, supersede it with a
   new one).
5. Report the file path you created. Do not commit it yourself unless asked.
