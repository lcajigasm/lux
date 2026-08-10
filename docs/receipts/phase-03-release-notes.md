# Phase 3 release notes — Sintaxis, navegación y paleta

Commits from `2b9c8f2` (end of Phase 2) to `HEAD` (9 commits).

## Highlights

- **`lux-syntax`**: incremental tree-sitter parsing across all 18 vendored
  grammars (`docs/adr/0008-tree-sitter-grammars-compiled-in.md`),
  token/scope-based themes (Lux Dark, Lux Light, high-contrast, all
  WCAG-verified), bracket pair colorization, syntax-derived code folding,
  and auto-indent by language — none of it heuristic string-scanning, all
  of it walking the real parse tree.
- **`lux-search`**: a `nucleo`-backed fuzzy matcher
  (`docs/adr/0009-search-and-fuzzy-matching.md`), generic over both plain
  file paths and arbitrary structs via a key function — reused by both
  Goto Anything and the command palette instead of two separate matchers.
- **`lux-ui`**: five new modules, each the tested core/state layer for a
  Phase 3 UI deliverable — `goto_anything` (fuzzy file search, `:line`
  suffix, recent-file history, preview), `command_palette` (a single
  command registry with `when`-clause context keys, structurally
  guaranteeing every command a menu would show is also in the palette),
  `search_bar` (navigation and replace on top of `lux-core`'s existing
  find/replace engine), `tabs` (open/close/pin/reorder/reopen-last-closed),
  `status_bar` (cursor/language/encoding/EOL/indentation, each backed by a
  real registered command — "all clickable" by construction), and
  `minimap` (density-bar rows, downsampled without empty padding, never
  shaping a single glyph).
- **Performance budgets measured for the first time**, with two real fixes
  that came out of measuring rather than assuming: `Document::highlight`
  was rescanning the *entire* tree on every call (fixed with a new,
  range-scoped `Document::highlight_range`), and the highlight `Query` was
  being recompiled from its `.scm` source on every call, which turned out
  to be the dominant cost (fixed by caching it once per `Document`). See
  `docs/PERFORMANCE.md`'s Phase 3 measurement history.

## Findings during close-out, not just numbers filled into a table

- **`Document::highlight` scanned the whole tree, not the edited or
  visible range** — fine for a one-shot golden-token test, wrong for a
  per-keystroke UI call on a large file. Measured at ~24.6 ms for
  reparse+highlight after one keystroke in a 20k-line file, well over the
  16 ms frame budget. `Document::highlight_range(source, byte_range)` now
  exists for exactly this; a simulated 50-line visible-range scope
  measures at ~2.3 ms.
- **Compiling the highlight query was the dominant cost, not the tree
  traversal.** Caching it (`Document::highlight_query`, built once in
  `Document::new`) cut both the whole-document and range-scoped numbers
  roughly in half again, on top of the range-scoping fix.
- **Goto Anything's "first results < 30 ms" isn't separately measurable**
  for the synchronous `nucleo::Pattern::match_list` this project chose
  (ADR 0009) — one call produces the complete result set. Measured
  against the stricter 150 ms full-ranking budget instead: 1.34 ms for
  100,000 files, comfortably inside it.
- **`@symbol` and `#text` (Goto Anything suffixes) are deferred, not
  missing** — they need Phase 7's LSP client and Phase 4's Find in Files
  respectively; building them now would mean throwaway infrastructure or
  pulling those phases forward. See
  `docs/adr/0014-goto-anything-symbol-text-suffixes-deferred.md`.

## Known gaps (honest, documented, not silently deferred)

- **No Phase 3 capability is wired into a real, visible UI.** Nothing
  from this phase paints anything different on screen from what Phase 2
  already showed: no syntax colors, no `Cmd+P`/`Cmd+Shift+P` dialogs, no
  search bar, no tabs, no status bar, no minimap. Every module above is
  the tested core/state layer a real widget would be built on, not the
  widget. This is the same "library capability, not the UI" scope note
  every individual deliverable's `docs/STATUS.md` entry already carries —
  called out here together because, taken as a whole, it means **this
  release looks identical to `v0.1.0` in the actual running editor**.
  Raised explicitly to the project owner before tagging (same kind of
  decision gate Phase 2's own closing report used); decision: ship
  `v0.2.0` now as the engineering-complete state of this phase, with UI
  wiring for all six interactive deliverables as the explicit focus of
  the work that follows.
- Windows and Linux remain `not-started` for the whole phase — same
  constraint as every phase before it (no dev environment available).
- Auto-indent's computed indent level isn't wired to any real editor
  command (no Enter-key/reindent-selection UI exists).
- Goto Anything has no real project file index yet (candidate lists are
  caller-supplied) — that's `lux-workspace`, Phase 4.
- The status bar's five commands (`status_bar.goto_line`,
  `.change_language`, `.change_encoding`, `.change_eol`,
  `.change_indentation`) are registered but have no real implementation
  behind them yet.

## Commits

```
328421e docs: close out Phase 3 -- STATUS, performance history, ADR, and receipts
f1f9d14 test(benches): add Phase 3 performance budget benchmarks
b52a9fd feat(lux-ui): add Goto Anything, command palette, search bar, tabs, status bar, and minimap
bfaea22 feat(lux-search): add nucleo-backed fuzzy matching
8fb5275 feat(lux-syntax): add auto-indent, range-scoped highlighting, and cached highlight query
914fb80 Add bracket pair colorization and syntax-based code folding
63f6955 Add token/scope-based syntax themes with WCAG-verified contrast
c121315 Add tree-sitter syntax highlighting support to lux-syntax
2b9c8f2 fix(ci): install libfontconfig1-dev on ubuntu runners
```
