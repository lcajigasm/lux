# 0014. Goto Anything: `@symbol` and `#text` suffixes deferred to their owning phases

## Status

Accepted.

## Context

Phase 3 deliverable 6 (`LUX_PROMPT_MAESTRO.md`) names four things in one
sentence: "fuzzy sobre ficheros del proyecto con `nucleo`, con sintaxis de
sufijo estilo Sublime (`fichero:línea`, `@símbolo`, `#texto`), historial de
recientes y previsualización." The Phase 3 work that landed
(`docs/receipts/phase-03-goto-anything-nucleo-sufijos-ficherolinea.md`)
built the fuzzy core and the `:línea` suffix only, and left the rest for
this note to resolve as part of closing the phase.

Of the remaining three, two — `@símbolo` and `#texto` — aren't just
unbuilt UI, they need capabilities this project's own phase plan assigns
to *later* phases:

- **`@símbolo`** (jump to a symbol by name) needs a real symbol index —
  function/type/variable names extracted from source. The two credible
  sources for that are LSP `workspace/symbol` (Phase 7 — `lux-lsp`,
  `docs/adr/0011-lsp-client-libraries.md`) or a tree-sitter-based outline
  query per language, which doesn't exist anywhere in `lux-syntax` today
  (Phase 3 built highlighting, not symbol extraction) and would be a new
  per-grammar query set to design, name, and vendor for all 18 languages —
  real new scope, not a two-line addition.
- **`#texto`** (jump to a line containing arbitrary text, across the whole
  project) needs full-text search across every project file. That's
  Phase 4 deliverable 4, named explicitly: "Find in Files con el motor de
  ripgrep embebido (`ignore` + `grep-searcher` + `grep-regex`)." Building
  a second, parallel full-text search path inside `lux-ui::goto_anything`
  now would mean either a throwaway implementation that gets replaced when
  Phase 4 lands, or accidentally building Phase 4 early.

Golden rule #7 is explicit: "Scope is closed... Do not extend the roadmap
on your own initiative." Implementing either suffix now means picking one
of: (a) building throwaway infrastructure duplicated by a later phase, (b)
quietly pulling a later phase's deliverable forward, or (c) a fake/stub
suffix that matches syntax but returns nothing real — which golden rule #1
rules out directly ("a stub that returns plausible data is worse than a
compile error").

## Decision

`@símbolo` and `#texto` are deferred: `@símbolo` to Phase 7 (once `lux-lsp`
exists and can serve `workspace/symbol`, or once a tree-sitter outline
query set is deliberately scoped as its own piece of work), `#texto` to
Phase 4 deliverable 4 (once the ripgrep-backed Find in Files engine
exists, `lux-ui::goto_anything` can call the same engine for the `#`
prefix instead of building a second one).

`lux-ui::goto_anything::Query::parse` already only recognizes the
`:line` suffix; an input starting with `@` or `#` is treated as literal
fuzzy-match text against filenames today (falls through to the general
case, matching nothing useful, rather than erroring) — that's acceptable
as an interim behavior, not a design decision to revisit until one of the
two dependencies above actually lands.

Recent-file history and preview, the other two items named in the same
deliverable sentence, have no such cross-phase dependency — history is
pure state (same "logical clock" recency tracking already built for
`command_palette::CommandRegistry`), and preview only needs
`lux-core::buffer::Buffer::open` to read the target file's content, both
achievable inside Phase 3's own scope. They're implemented as part of
closing this phase, not deferred; see `docs/STATUS.md`'s Goto Anything
entry for what shipped.

## Alternatives considered

Implementing a minimal tree-sitter outline query now, just for `@symbol`
— rejected: "minimal" for one language would still mean a real per-grammar
query design decision repeated 18 times to be honest about scope (the same
reasoning ADR 0008 already applied to the highlight query set), which is
Phase 7/a dedicated outline-query deliverable's job, not something to
half-do inside a Phase 3 closing pass.

Shelling out to `ripgrep`/reusing `grep` crates directly inside
`goto_anything` for `#texto` instead of waiting for `lux-search`'s Find in
Files — rejected: `lux-search` is explicitly where `ignore` +
`grep-searcher` + `grep-regex` are meant to live (`docs/ARCHITECTURE.md`),
and building the same integration twice (once ad hoc here, once for real
in Phase 4) is duplicated work for no benefit.

## Consequences

`docs/STATUS.md`'s Goto Anything row stays `not-started` as a user-facing
feature (no UI exists regardless), but its prose should be read alongside
this ADR: the reason `@symbol`/`#text` are missing isn't an oversight, and
completing them isn't a Phase 3 punch-list item — check back here when
Phase 4 or Phase 7 lands before re-opening `goto_anything::Query` to add
either prefix, so the implementation reuses `lux-search`'s Find in Files
and `lux-lsp`'s symbol source respectively instead of inventing parallel
ones.
