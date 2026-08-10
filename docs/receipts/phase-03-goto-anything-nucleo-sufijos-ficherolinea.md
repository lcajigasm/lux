# Receipt: Goto Anything (core + file:line suffix)

- Phase: 03
- Crate(s): lux-search, lux-ui
- Goal: The "Goto Anything (nucleo + sufijos fichero:linea)" deliverable of
  Phase 3 ("Syntax, navigation, and palette") from `LUX_PROMPT_MAESTRO.md`
  is complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-ui goto_anything::`
- Iterations: 1 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** Deliverable 6 of Phase 3 in `LUX_PROMPT_MAESTRO.md` bundles
  several capabilities into a single sentence: fuzzy matching with
  `nucleo`, `file:line`/`@symbol`/`#text` suffixes, recent-files
  history, and preview. This loop's goal explicitly scopes it down to
  "core + file:line suffix": the fuzzy ranking engine backed by
  `nucleo`, plus parsing of the Sublime-style `:line` suffix.
  `@symbol`, `#text`, history, and preview are out of scope, as is the
  real UI (`Cmd+P`, dialog, keybinding) and the real project file index
  (that's `lux-workspace`, Phase 4, not yet built). That's why
  `docs/STATUS.md` keeps the "Goto Anything" row as `not-started`
  (rule 1: a feature exists whole or it does not exist) and documents
  the internal progress in prose instead of marking the row as complete.

  **Where the matching engine lives.**
  `docs/adr/0009-search-and-fuzzy-matching.md` already decided, before
  this loop, that `nucleo` lives in `lux-search` ("`lux-search` embeds ... `nucleo`
  for fuzzy matching in Goto Anything and the command palette") — not in
  `lux-ui`. That decision was followed: `lux-search::fuzzy::rank_paths` is
  the only new function there, a thin wrapper over
  `nucleo::pattern::Pattern::match_list` with
  `Config::DEFAULT.match_paths()` (the `nucleo` preset designed for
  paths: `/` scores as a segment boundary). `lux-ui::goto_anything` only
  knows about suffix parsing and composes the result; it does not
  reimplement any of the matching. `lux-ui` was already allowed to depend
  on `lux-search` per `docs/ARCHITECTURE.md` (a domain crate, not
  UI/render).

  **Why `Pattern::match_list` and not the high-level `Nucleo`.** The
  `nucleo` crate exposes two layers: `Nucleo<T>`, designed for
  interactive *pickers* (background threads, `tick()`, snapshots) —
  necessary when you need to rank hundreds of thousands of files without
  blocking a UI thread, but not something this loop should set up (there
  is no UI or real project index yet); and `Pattern::match_list`, the
  synchronous, single-threaded function that `nucleo`'s own
  documentation describes as the recommended path for small lists and
  which gives deterministic results, with no threads or timers, ideal
  for tests. The split between the two is documented in ADR 0009 itself
  and in the header of `fuzzy.rs`: when the real Phase 4 index exists
  and volume justifies it, migrating to `Nucleo<T>` is a local change
  to `lux-search`, not to the suffix syntax in `lux-ui`.

  **Parsing the `file:line` suffix.** `Query::parse` looks for the
  last `:` with `rsplit_once`; it only treats it as a line suffix if
  what follows parses as a positive integer (`line @ 1..`, 1-based like
  Sublime). Anything else — no colon, `:0`, `:abc`, or a trailing `:`
  with no digits yet (the transient state while the user keeps typing)
  — is left as literal search text instead of being discarded or
  failing, so that progressive typing never drops what's already been
  typed.

  **Tests, not just coverage.** ADR 0009 explicitly asks for
  "characterization tests" of `nucleo`'s ranking behavior, so that a
  fork or an in-house replacement is cheap if needed — that's why
  `lux-search::fuzzy` has its own tests (path-boundary bonus, `Smart`
  case sensitivity, gap between characters, empty query) in addition to
  the ones required by the `lux-ui` gate. `goto_anything::` in `lux-ui`
  covers suffix parsing (plain text, valid suffix, `:0` rejected,
  non-numeric suffix, trailing `:` with no digits, suffix with no file
  part) and full composition (`search`): line attached to each result,
  non-matching candidates excluded, empty query after the suffix returns
  all candidates unscored (the "jump to line N of the current file"
  case).

  **Real verification.** `cargo test -p lux-ui goto_anything::`: 10/10
  passed, 0 ignored. `cargo test -p lux-search fuzzy::`: 6/6 passed.
  Full repo gate green: `cargo run -p xtask` (fmt, clippy `-D warnings`
  across the whole workspace — fixed a real `collapsible_if` flagged by
  clippy in `Query::parse`, `cargo test --workspace`: 235 tests in
  `lux-core` plus the rest of the workspace with no regressions,
  `cargo deny check` — the MPL-2.0 license of `nucleo`/`nucleo-matcher`
  passes the license check, no new duplicates —, file limits, `lux-core`
  isolation). `crates/lux-search/src/fuzzy.rs` (75 lines) and
  `crates/lux-ui/src/goto_anything.rs` (195 lines) well below the
  600-line limit; no new struct exceeds 3 fields. Verified on macOS
  only, the same limitation documented throughout the rest of
  `docs/STATUS.md`.

  **What's out of scope for this deliverable** (the rest of Phase 3,
  untouched): `@symbol` and `#text` suffixes, recent-files history,
  preview, the real `Cmd+P` UI (no interactive UI exists in `lux-ui`
  yet, only passive widgets), the real project file index
  (`lux-workspace`, Phase 4), command palette, search/replace bar,
  tabs/status bar, minimap. Left untouched, neither modified nor
  committed, was the unrelated work already pending in the working tree
  when this loop started (`crates/lux-syntax/src/lib.rs` modified,
  `crates/lux-syntax/src/indent_rules.rs` and the auto-indent receipt
  untracked) — that is the previous deliverable, already resolved and
  with its own receipt, only pending a commit from whoever closes it.
