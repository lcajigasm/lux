# Receipt: Virtualized tree explorer (deliverable 2 of Phase 4)

- Phase: 04
- Crate(s): lux-ui
- Goal: The "Virtualized tree explorer" deliverable of Phase 4 ("Projects,
  files and global search") from `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-ui file_tree::`
- Iterations: 3 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text: "Tree file explorer,
  virtualized, no item limit (v0 was a flat list of 400), with creation,
  renaming, deletion to the system trash, drag and drop, and revealing the
  active file." Only this bullet was in scope — not `lux-workspace`'s
  multi-root/`.lux-project` (already closed, previous loop), native
  watchers, find in files, or the incremental index (Phase 4 deliverables
  1 and 3-6). `docs/STATUS.md`'s "File tree explorer" row stays
  `not-started` (rule 1): like every other `lux-ui` deliverable so far
  (`tabs`, `command_palette`, `minimap`, `goto_anything`, `search_bar`),
  this is the state/data layer a tree view widget would render and drive —
  no pixels, no mouse/keyboard/drag event handling exists in `lux-ui` yet.

  **"No item limit" is a lazy-loading guarantee, not a magic number.**
  v0's actual bug was a flat list eagerly built and capped at 400 entries.
  `FileTree` never walks a directory it isn't asked to: `open()` reads only
  the root's immediate children, and every other directory's `children`
  stays `None` (unread) until `expand()` is called on it. Opening a
  100k-file project only ever touches the top-level entries; nothing
  recursive happens up front. This is the same technique already used for
  `EditorView` (only visible lines are shaped) and `minimap` (density bars,
  never real glyphs) — read what's needed, not what exists.

  **"Virtualized" is a real windowed slice, not just lazy loading.**
  `rows()` flattens the *currently expanded* subtree (bounded by what the
  user opened, not by total project size) and caches it (`rows_dirty`,
  invalidated by every structural change); `visible_rows(first_row,
  visible_count)` slices that cache the same way `EditorViewFrame`'s
  `visible_line_range` and `minimap`'s row-bucketing already do for their
  own widgets — a caller only ever needs the viewport's rows in hand, not
  the whole tree. Verified directly in `visible_rows_slices_by_viewport`,
  including the out-of-range-clamped case.

  **"Drag and drop" reuses the exact interpretation `tabs::TabBar` already
  established for the same words in Phase 3.** That receipt's tab strip
  treats "drag & drop" as "the primitive a drag gesture reports its drop
  position through" (`move_tab`), not real OS pointer-event plumbing —
  because no interactive UI exists yet to generate real drag events either.
  `FileTree::move_node(id, new_parent)` is the same primitive for the tree,
  with one real difference: `TabBar::move_tab` only reorders an in-memory
  `Vec` (tabs aren't real files), but tree nodes are — `move_node` is
  backed by a genuine cross-directory `fs::rename`, refuses to move a
  directory into itself or one of its own descendants
  (`move_node_into_its_own_descendant_errors`), and refuses to move the
  root (there's nothing to reparent it into).

  **Delete is real, not simulated — the one place this deliverable
  couldn't reuse the "state only, no I/O" pattern of every prior `lux-ui`
  widget.** Rule 1 forbids a delete button wired to something that only
  looks like deletion. "Deletion to the system trash" specifically (not
  `fs::remove_file`/`remove_dir_all`) needed a real OS trash integration, so
  a genuine dependency was added: `trash` 5.2.6, the only actively
  maintained, real-adoption, cross-platform (Windows/macOS/Linux,
  freedesktop trash spec) crate for this — recorded in
  `docs/DEPENDENCIES.md`, no dedicated ADR (unlike `.lux-project`'s JSON-
  vs-JSONC choice in the previous loop, there was no real alternative to
  weigh it against, so there's no decision to write up beyond the
  dependency-table entry). `delete_to_trash_removes_the_file_from_disk_and_
  from_the_tree` performs an actual system-trash deletion during the test
  run, not a mock — the same "test the real thing" standard `Buffer::save`'s
  own tests already hold themselves to.

  **`create`/`rename`/`move_node` re-derive sort order instead of
  re-reading the directory from disk.** A fresh `load_children` call
  produces "directories first, then case-insensitive name," but forcing a
  full re-read after every single-item mutation would be wasteful and
  would also undo any of the caller's own already-loaded, already-expanded
  descendant state elsewhere in the tree. `node_sort_key` is the one
  source of truth for that ordering, reused by the initial load and by
  `resort_children` after each structural change (extracted from the first
  draft's duplicated per-callsite `sort_by` closures during the clippy
  pass below — `sort_by_key` plus one shared method, not two near-copies).

  **`reveal` only expands ancestors, not the target itself.** The first
  draft expanded every path component including the last, which meant
  "reveal a directory" side-effected into "reveal and open it" — not what
  "revelado del fichero activo" (reveal the active file) asks for. Fixed by
  only calling `expand` on intermediate components
  (`i + 1 < components.len()`), so the target node is *found* and returned
  without changing its own expansion state. Caught before it needed a test
  fix — `reveal_expands_ancestors_and_returns_the_target_id` asserts on the
  ancestor chain's row names, which would have included the target
  directory's own children had the bug shipped.

  **A real iteration, not a first-try pass — three clippy findings fixed
  after the first green test run.** `cargo test -p lux-ui file_tree::`
  passed 19/19 on the first attempt, but `cargo clippy --workspace
  --all-targets -- -D warnings` then failed on `collapsible_if` (three
  call sites doing `if let Some(parent) = ... { if let Some(n) = ... { if
  let Some(children) = ... { retain(...) } } }`, each a real duplicate of
  the same "drop a child from a loaded children list" logic) and
  `clippy::sort_by` where `sort_by_key` was the correct call (two sites).
  Fixed by extracting one shared `remove_child(parent, child)` helper
  (collapsing three call sites into one, not three separate `if let`
  chains) and switching both sorts to `sort_by_key(|&id|
  self.node_sort_key(id))`. Re-ran the full gate clean afterward — the
  fixes changed structure, not behavior, so no test needed updating.

  **A second, unrelated failure: the file grew past the 600-line limit.**
  `scripts/check-limits.sh` flagged `file_tree.rs` at 824 lines (impl +
  19 tests in one file) against golden rule #4's cap. Split into a
  directory module — `file_tree/mod.rs` (495 lines: the real impl) and
  `file_tree/tests.rs` (331 lines, loaded via `#[cfg(test)] mod tests;`,
  same structure `editor_view/` already uses for its own submodule) —
  rather than trimming test coverage to fit, since every test in the file
  earns its place (each covers a distinct operation or edge case, none is
  redundant). Both files re-verified independently under the limit; no
  behavior changed, `cargo test -p lux-ui file_tree::` still 19/19 after
  the split.

  **Actual verification.** `cargo test -p lux-ui file_tree::`: 19/19
  passed, 0 ignored, on the final run (0 failed on the very first run of
  the tests themselves — the two iterations above were both caught by
  `cargo run -p xtask`'s other gates, not by the test command itself, so
  the goal's specific gate command never regressed). Coverage: root open
  on both a directory and a plain file, directory-first/case-insensitive
  sort order, expand/collapse round-tripping the same cached node ids
  (proof children aren't re-read), collapse hiding but not forgetting
  descendant rows, viewport slicing (normal and clamped), create (file and
  directory, plus the under-a-file-parent error), rename (updating both
  the renamed node's path and every already-loaded descendant's, plus the
  root-is-immutable error), move (real cross-directory move, the
  move-into-own-descendant error, the move-root error), delete-to-trash
  (a real deletion, plus the double-delete/unknown-id error), and reveal
  (ancestor expansion, plus the outside-the-root `None` case). Full repo
  gate also run and green: `cargo run -p xtask` — `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets -- -D warnings` (zero warnings
  after the fixes above); `cargo test --workspace` (all crates, no
  regressions — `lux-core`'s 235 and `lux-workspace`'s 21 from the previous
  loop still pass); `cargo deny check` (advisories/bans/licenses/sources
  all `ok`, no new `deny.toml` entries needed for `trash` or its
  macOS-only `objc2`/`objc2-foundation` transitive deps); `scripts/check-
  limits.sh` (both `file_tree/mod.rs` and `file_tree/tests.rs` under 600
  lines; `Node` — the largest new struct — has 7 fields, well under 25);
  `scripts/check-core-isolation.sh` (doesn't apply — only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only — same
  documented limitation as the rest of `docs/STATUS.md`; the `trash`
  crate's Windows/Linux backends are unexercised here.

  **What's out of scope and untouched:** native `notify`-based watchers,
  Find in Files, replace-in-project, the incremental index, sessions/
  recovery, and huge/binary file handling (Phase 4 deliverables 3-6);
  wiring any of this into the `lux` binary or a real UI (rendering,
  mouse/keyboard/real drag events); reload-on-external-change (watchers'
  job, deliverable 3); Windows/Linux trash verification.
