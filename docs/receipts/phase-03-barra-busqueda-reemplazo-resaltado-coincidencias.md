# Receipt: Search/replace bar + match highlighting (deliverable 8)

- Phase: 03
- Crate(s): lux-ui
- Goal: The "Search/replace bar + match highlighting" deliverable of Phase 3
  ("Syntax, navigation and palette") from `LUX_PROMPT_MAESTRO.md` is complete
  and its gate passes with no failure and no ignored test.
- Verify: `cargo test -p lux-ui search_bar::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Actual scope of this deliverable.** The full text is "Search and
  replace bar in the buffer, with regex, and navigation between results
  with highlighting of all matches". Before writing anything,
  `crates/lux-core/src/find_replace.rs` was reviewed — and it turns out
  the entire engine already exists and is complete since Phase 1: `Query`
  (literal/case-insensitive/whole-word/regex, all four via a single
  compiled `regex::Regex`), `find_all`, `find_next`, `replace_one`,
  `replace_all`, all transactional/undoable. `docs/STATUS.md` already
  marked "Find/replace in buffer" as `implemented` in the Phase 1
  section. What didn't exist was the UI part: which result is "active"
  right now, and keeping that in sync as the buffer changes. That is
  the actual work of this loop — `crates/lux-ui/src/search_bar.rs`, a
  new module, zero new matching logic.

  **Why navigate by index instead of reusing `find_next`.** `lux-core`
  also exposes `find_next(buffer, query, after: Position)`, which finds
  the next match starting from a position, with wraparound — but it is
  designed for "jump to the result closest to the cursor", not for a bar
  that already needs the full list of matches in memory in order to
  highlight all of them. Since that list always exists
  (`SearchBar::matches`), navigating is just incrementing/decrementing an
  index modulo the count — without re-running the regex on every
  `next_match()`/`previous_match()`, and without needing to convert
  positions. `find_next` is left untouched and remains the correct
  function for other cases (e.g. a future "search from cursor" in the
  real UI wiring phase).

  **`next`/`previous` are named `next_match`/`previous_match`.** First
  attempt used the short names; clippy (`should_implement_trait`)
  rejected them because `fn next(&mut self) -> Option<T>` gets confused
  with `Iterator::next` — a more specific name, without implementing
  `Iterator` (which wouldn't make sense: the "iteration" here has
  bidirectional navigation state with wraparound, not a linear
  single-pass consumption).

  **An invalid, half-typed regex does not clear the results.**
  `set_query` calls `find_all` with the new query *before* mutating
  `self`; if it fails (incomplete regex, e.g. typing `"foo("` letter by
  letter), the error propagates and the previous state (last valid
  query + its matches) stays as is, instead of being cleared. Same
  criterion already used for the `:line` suffix in Goto Anything: an
  invalid transitional state while the user is typing should not
  discard the last good result.

  **`replace_current` advances "for free".** By replacing the match at
  index `i` and recalculating `matches` against the already-mutated
  buffer, that occurrence disappears from the list and everything after
  it shifts one index back — leaving `current` untouched after the
  refresh means it points, with no special case at all, to what used to
  be the *next* match. Explicitly verified in
  `replace_current_replaces_and_advances_to_what_was_the_next_match`.

  **Actual verification.** `cargo test -p lux-ui search_bar::`: 11/11
  passed, 0 ignored (match calculation + starting on the first one, no
  matches, combined regex/case-insensitive options, invalid regex
  preserves previous state, wraparound in both directions, no matches
  returns `None`, `refresh` picks up edits made outside the bar, single
  replace with automatic advance, single replace with no active match is
  a no-op, single replace is undoable via the returned `Transaction`,
  global replace leaves the list empty). One real failure during
  development: two test assertions with a manually miscalculated range
  (`17..20` instead of `16..19` for the third "cat" in
  `"cat bat cat hat cat"`) — an arithmetic error in the test, not in the
  code; fixed after carefully counting the indices. Full repo gate
  green: `cargo run -p xtask` (fmt — applied after writing the file;
  clippy `-D warnings` across the whole workspace — the only real
  finding was the `next`/`previous` one above; `cargo test --workspace`:
  235 tests in `lux-core` + the rest of the workspace with no
  regressions — including the 26 `find_replace::` tests reverified
  intact; `cargo deny check`; file limits — `search_bar.rs` at 283
  lines, well under the 600 limit; `lux-core` isolation). `SearchBar`
  has 3 fields. No new dependency was added: everything is built on top
  of what `lux-core` already exposed (`buffer`, `find_replace`,
  `history`, `selection`). Verified only on macOS, same documented
  limitation as the rest of `docs/STATUS.md`.

  **What is out of scope for this deliverable** (rest of Phase 3,
  untouched): the actual bar UI (`Cmd+F`/`Cmd+H`, dialog, keybindings —
  Phase 5), the actual highlighting pass in `EditorView` (no interaction
  UI exists yet in `lux-ui`, same state left by the two previous
  receipts of this phase), tabs/status bar, minimap. Left untouched, not
  touched or committed, the pending work from previous deliverables that
  was already in the tree when this loop started (auto-indent in
  `lux-syntax`, and Goto Anything / command palette from the two
  previous loops, each with its own receipt, still uncommitted).
