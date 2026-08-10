# Receipt: Command palette unified with menus (deliverable 7)

- Phase: 03
- Crate(s): lux-search, lux-ui
- Goal: The deliverable "Command palette unified with menus" of Phase 3
  ("Syntax, navigation and palette") of `LUX_PROMPT_MAESTRO.md` is
  complete and its gate passes with no failure and no ignored test.
- Verify: `cargo test -p lux-ui command_palette::`
- Iterations: 1 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The full deliverable 7 reads: "single command registry
  with categories, ranking by recent use and *context keys* (`when`
  clauses) for contextual enablement. Every menu command must be
  in the palette". There is still no interactive UI in `lux-ui` (only
  passive widgets, the same state left by the Goto Anything receipt), so
  no actual menu exists to "unify". For that reason, just as with
  Goto Anything, `docs/STATUS.md` keeps the "Command palette" row at
  `not-started` (rule 1) and documents in prose what does exist: the
  single registry, the fuzzy + recency ranking, and the `when` clause
  parser — the "unification with menus" part is resolved by
  construction (see below), not with a UI that isn't due to be
  built yet.

  **How "every menu command must be in the palette" is resolved
  without any menu existing.** A literal reading of "unified with menus"
  when there are no menus would be impossible to verify. The reading that
  *is* verifiable now: make it structurally impossible for a list
  of menu commands to exist separately from the registry. `CommandRegistry` is the
  only place in all of `lux-ui` where a command can be defined — there is no
  "menu item" type anywhere in the crate. When a real menu exists, it will have
  no other source to pull its items from than
  `CommandRegistry::commands()`, the same registry the palette searches.
  That is exactly what v0 got wrong (two hand-maintained lists that
  diverged) and what this makes impossible by design, not by discipline.
  The test `no_separate_menu_list_every_registered_command_is_in_the_palette`
  documents this property: everything registered shows up in an unfiltered
  search, because there is no other registration path.

  **Ranking engine: `nucleo` is reused, not reimplemented.**
  `docs/adr/0009-search-and-fuzzy-matching.md` already assigns `nucleo` to
  `lux-search` for "Goto Anything and the command palette" — literally
  this deliverable. `lux-search::fuzzy::rank_paths` (from the previous loop)
  did not fit as-is: it uses `Config::match_paths()` (bonus for `/`) and operates
  on plain `&str`, but commands are structs and two commands can
  share a title (e.g. "Format Document" in two categories). `rank_by` was added,
  generic over any `T` via a `key: Fn(&T) ->
  &str` function, with the default `Config` (without the path bias, which makes
  no sense for command titles) — without duplicating `nucleo`'s
  scoring logic, only the entry point. `rank_paths` is left untouched.

  **Ranking by recent use: logical clock, not wall clock.** Instead of
  `SystemTime`/`Instant`, `CommandRegistry` carries a `u64` counter that
  increments on every `record_use`; only the relative order between
  commands matters, never real elapsed time, so a monotonic
  counter is strictly sufficient and makes the tests deterministic without
  any `sleep` or clock mock. With an empty query, the order is
  purely by recency (used ones first, never-used ones last in registration
  order). With a non-empty query, the primary criterion is the fuzzy
  score and recency only breaks ties on equal scores — so that an
  old command with a better match never ends up buried under a
  recent one with a worse match.

  **`when` clauses: a real parser, not an `if` with a single case.** A
  genuine boolean mini-language was implemented — `&&`/`||`/`!`/`(...)`
  with standard precedence (`!` > `&&` > `||`) — with its own tokenizer and
  recursive-descent parser (~90 lines), over a `Context` of
  boolean keys (absent = `false`, same as VS Code's `when`). Limiting the
  scope to "a single key, no operators" was considered (more
  ponytail), but that is not what the deliverable says ("*context keys*
  (`when` clauses)") nor what is really needed: a real contextual
  enablement rule almost always needs to negate or combine more
  than one condition (e.g. `editorFocus && !hasSelection`), so
  parentheses/precedence is not speculative scope, it is the minimum
  use case. An empty clause (`""`) is always true (command with no
  restriction); a malformed clause (unclosed parenthesis, dangling
  operator, unknown character) is an `Err(WhenParseError)`, never
  silently treated as "always true" or "always false" —
  both options would hide a real configuration error.

  **`register` can fail.** A duplicate command id returns
  `Err(DuplicateCommandId)` instead of silently overwriting or
  panicking — it is a plausible programming error (two features
  registering the same id) over the registry's only structural invariant
  (unique ids), so it deserves explicit handling, unlike
  cases that truly cannot occur.

  **Actual verification.** `cargo test -p lux-ui command_palette::`: 12/12
  passed, 0 ignored (duplicate registration, `when` with a loose key,
  negation/`&&`/`||`, `&&` precedence over `||`, parentheses
  overriding precedence, empty clause always true, malformed
  clauses rejected, `search` filtering by `when`, fuzzy ranking by
  title, recency order with empty query, recency tie-break with equal
  query, and the "single registry" invariant).
  `cargo test -p lux-search fuzzy::`: 8/8 passed (the 6 previous `rank_paths`
  tests untouched plus 2 new ones for `rank_by`). Full repo gate
  green: `cargo run -p xtask` (fmt — reformatting was needed after
  writing the file, applied with `cargo fmt`; clippy `-D warnings` across
  the whole workspace — a real `useless_conversion` was fixed
  (an unnecessary `.into_iter()` when passing a `Vec` to an
  `impl IntoIterator` parameter); `cargo test --workspace`: 235 tests in `lux-core` +
  the rest of the workspace with no regressions; `cargo deny check`; file
  limits — `command_palette.rs` comes in at 547 lines, within the
  600 limit but the closest to the project limit so far, noted here
  for whoever adds `@symbol`/`#text`/history to Goto Anything or extends
  this palette, in case the file needs splitting then;
  `lux-core` isolation). No new struct exceeds 4 fields
  (`CommandRegistry`). Verified on macOS only, the same limitation
  documented elsewhere in `docs/STATUS.md`.

  **What is out of scope for this deliverable** (the rest of Phase 3, untouched):
  the actual `Cmd+Shift+P` UI (there is still no interaction UI in
  `lux-ui`), the actual menu whose items would consume the registry,
  keybindings (Phase 5, `lux-config`), Goto Anything `@symbol`/`#text`/
  history/preview (previous deliverable, already closed with its
  own receipt), search/replace bar, tabs/status bar,
  minimap. Left untouched, neither edited nor committed, is the pending
  work from previous deliverables that was already in the tree when this loop
  started (`crates/lux-syntax/src/lib.rs` modified and
  `crates/lux-syntax/src/indent_rules.rs` untracked, from auto-indent;
  and the Goto Anything changes from the previous loop, already with their own
  receipt, also uncommitted).
