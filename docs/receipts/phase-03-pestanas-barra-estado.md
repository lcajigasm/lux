# Receipt: Tabs + status bar (deliverable 9)

- Phase: 03
- Crate(s): lux-ui
- Goal: The "Tabs + status bar" deliverable of Phase 3 ("Syntax,
  navigation, and palette") in `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-ui -- tabs:: status_bar::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The full text of deliverable 9: "tabs with drag & drop,
  pinning, closing, reopening the last closed one; status with cursor
  position, language, encoding, line endings, and indentation, all
  clickable." As with the rest of Phase 3, no real interactive UI exists
  in `lux-ui` yet (no tab rendering, no mouse/drag event handling) — so,
  same as the four previous receipts in this phase, this is the state
  layer a real UI would consume, not the UI itself. `docs/STATUS.md` keeps
  the "Tabs + status bar" row at `not-started` (golden rule #1), with the
  prose detailing what's built.

  **`tabs.rs`: `TabBar`.** `open`/`close`/`reopen_last_closed` (a LIFO
  stack — closing A then B and reopening twice returns B first, then A) /
  `pin`/`unpin`/`move_tab`/`activate`. Two explicit design decisions:
  - **Ids are minted by `TabBar` itself, never chosen by the caller**
    (an opaque `TabId(u64)`, internal `next_id`). Unlike
    `command_palette::Command` (whose id must be stable and chosen
    externally, since future keybindings will reference it), a tab has no
    such requirement, so an auto-generated id eliminates the whole class
    of "two tabs with the same id" instead of needing an `Err` `Result`
    to detect it — simpler than the command registry, deliberately,
    because the problem it solves is different.
  - **`pin`/`unpin` actively reorder** (moving the tab to the end of the
    pinned group / the start of the unpinned group) instead of just
    flipping a boolean — pinning without visual grouping would be a
    function that does nothing useful. `move_tab`, by contrast, doesn't
    enforce that grouping: it's the raw "move to position X" primitive a
    drag gesture reports, and most editors allow free manual interleaving
    during a drag.
  - `close` activates a sensible neighbor (the one to the right, or the
    one to the left if the last tab was closed) instead of leaving
    `active` dangling or jumping arbitrarily to the first tab.

  **`status_bar.rs`: reuses Phase 1 detection, doesn't reinvent it.**
  `lux-core::buffer` already exposes detected
  `eol()`/`indent()`/`encoding()` (the "EOL / indent / encoding
  detection" row in `docs/STATUS.md`, `implemented` since Phase 1). This
  module only formats them for display ("LF"/"CRLF"/"CR", "UTF-8"/"UTF-8
  BOM"/"UTF-16 LE"/"UTF-16 BE"/"Latin-1", "Tabs"/"Spaces: N") with
  exhaustive `match`es and no wildcard arm — if `lux-core` ever adds a new
  variant to `Eol`/`Encoding`/`IndentStyle`, this stops compiling instead
  of silently showing a wrong label. `language` is a `&str` the caller
  passes in (no language detection is wired into `lux-ui` yet; that's
  `lux-syntax`, outside this deliverable). Cursor position is formatted
  1-based ("Ln 2, Col 1"), the Sublime/VS Code convention, from
  `lux-core`'s 0-based `Position`.

  **"All clickable" resolved the same way the previous deliverable
  resolved "every menu command is in the palette": structurally, not by
  convention.** `StatusBarItem` has no constructor that allows creating an
  item without a `command_id` — there's no "label only" variant. And
  `status_bar::register_commands` registers those same five ids
  (`status_bar.goto_line`, `.change_language`, `.change_encoding`,
  `.change_eol`, `.change_indentation`) into a real `command_palette::
  CommandRegistry`. The test
  `every_built_status_bar_item_resolves_to_a_registered_command` verifies
  end-to-end that every `command_id` produced by `build()` shows up in the
  registry after `register_commands` — a click always resolves to a real
  command, never to a bare, unbacked string. This ties this deliverable to
  the command palette one (same crate, same registry) instead of
  inventing a second, parallel "action" mechanism.

  **Gate needs `--`.** `cargo test -p lux-ui tabs:: status_bar::` as
  written fails — `cargo test` only accepts one bare filter; two
  space-separated filters need `--` to reach the harness. Same case
  already documented in
  `docs/receipts/phase-03-bracket-pair-colorization-folding-real.md`. The
  command that actually ran: `cargo test -p lux-ui -- tabs::
  status_bar::`.

  **Real verification.** `cargo test -p lux-ui -- tabs:: status_bar::`:
  25/25 passed, 0 ignored (15 `tabs::` tests: open/activate, close with
  right/left/no neighbor reactivation, closing an unrelated tab doesn't
  change the active one, closing an unknown id, reopening in LIFO order,
  reopening with nothing closed, pin groups at the start, pinning twice
  is a stable no-op, unpin moves to the start of the unpinned group,
  move_tab reorders by id, move_tab clamps an out-of-range index to the
  end, moving/activating an unknown id both return `false`; 10
  `status_bar::` tests: every `Eol`/`Encoding` variant, tabs/spaces
  indentation formatting, 1-based cursor, the buffer's real EOL/encoding,
  a caller-supplied language label, registering the five commands,
  duplicate registration fails, and the build+registry integration).
  Full repo gate green: `cargo run -p xtask` (fmt with no diffs on the
  first try; clippy `-D warnings` across the whole workspace with no
  findings; `cargo test --workspace`: 235 tests in `lux-core` plus the
  rest of the workspace with no regressions, 83 total in `lux-ui` (58
  previous + 25 new); `cargo deny check`; file limits — `tabs.rs` at 326
  lines, `status_bar.rs` at 210 lines, both well under the 600 limit;
  `lux-core` isolation). No new struct exceeds 5 fields (`StatusBar`). No
  new dependency was added. Verified on macOS only, same constraint
  documented throughout the rest of `docs/STATUS.md`.

  **What's out of scope for this deliverable** (the rest of Phase 3,
  untouched): the real tabs/status bar UI (rendering, mouse/drag event
  handling — no interactive UI exists in `lux-ui` yet, same state left by
  the four previous receipts in this phase), the real commands behind
  `status_bar.change_language`/`.change_encoding`/`.change_eol`/
  `.change_indentation`/`.goto_line` (registered but not implemented —
  future actions, not this deliverable's job), the minimap. Left
  untouched, uncommitted, was the pending work from earlier deliverables
  that was already in the tree when this loop started (auto-indent in
  `lux-syntax`, and Goto Anything / command palette / search bar from the
  three previous loops, each with its own receipt, still uncommitted).
