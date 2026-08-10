# Receipt: Incremental index fed by watchers (deliverable 6 of Phase 4)

- Phase: 04
- Crate(s): lux-workspace
- Goal: The "Incremental index fed by watchers" deliverable of Phase 4
  ("Projects, files and global search") from `LUX_PROMPT_MAESTRO.md` is
  complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-workspace index::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text is one line: "Incremental
  workspace index fed by the watchers, not periodic rescans." Only this
  bullet was in scope — not sessions/recovery or huge/binary file handling
  (Phase 4 deliverables 7-8), and not rewiring `find_in_files` or a future
  Goto Anything to actually query the index instead of walking fresh —
  that's separate integration work with no owner yet (no UI, no unified
  `lux` binary wiring exists for any Phase 4 piece so far). `docs/
  STATUS.md` gets a scope note, not a moved capability row: the phase's
  capability table never had a dedicated row for the index (7 rows for 8
  deliverables — same situation deliverable 1's multi-root data model was
  in), and this is infrastructure a future consumer would key off, not a
  user-visible capability by itself.

  **The literal requirement — "fed by watchers, not periodic rescans" — is
  what the design is organized around, not just a docstring claim.**
  `WorkspaceIndex::build` does exactly one real recursive scan
  (`ignore::WalkBuilder`, same `.gitignore`-respecting, `require_git
  (false)` configuration `lux_search::find_in_files` already established
  in the loop two before this one) to establish the baseline file set.
  Every update after that — `apply_changes` — is a plain function over a
  batch of `lux_workspace::watcher::FileChange`s the caller already
  drained from a `FileWatcher`; there's no method on `WorkspaceIndex` that
  re-walks anything, so there's structurally nothing to accidentally call
  on a timer. This mirrors `lux_search::replace_preview::build_preview`'s
  "consume another module's already-produced output" shape from the loop
  right before this one, applied to the watcher's output instead of
  `find_in_files`'s.

  **A real end-to-end test, not just unit tests of `apply_changes` fed
  synthetic `FileChange` values.** Nine of the ten tests do exactly that
  (fast, deterministic, no timing dependency) — but "fed by watchers" is a
  claim about real integration with `FileWatcher` from two loops ago, and
  a synthetic `FileChange { path, kind: Created }` built by hand proves
  the types line up, not that the actual wiring works. So the tenth test,
  `index_stays_correct_when_fed_by_a_real_watcher`, starts a genuine
  `FileWatcher`, performs a real `fs::write`, and drains the watcher's
  actual `recv()` output straight into `apply_changes` in a loop until the
  index reflects the change or a 5 s deadline trips — the same real-
  backend testing standard the watcher's own receipt (two loops ago) and
  the file tree's `delete_to_trash` test (three loops ago) already held
  themselves to, applied here to prove the two modules actually compose
  correctly, not just that their types are compatible.

  **Directory creates are silently not indexed, and removals don't need to
  know what they used to be — both deliberate, to avoid touching
  `watcher.rs` at all.** `FileChange` (built in the watcher-receipt loop)
  carries no `is_dir` flag. Rather than extend that struct for this
  deliverable's sake, `apply_changes` decides file-vs-directory itself
  with a fresh `Path::is_file()` stat on `Created`/`Reload` (the same kind
  of real-time stat `watcher::classify` already does for its own FSEvents-
  quirk workaround), and treats `Removed` as an unconditional
  `HashSet::remove` — removing a path that was never indexed (it was a
  directory, or already gone) is a harmless no-op on a set, so there's no
  need to know in advance which case applies. Verified directly
  (`apply_changes_created_for_a_path_that_is_not_a_file_is_a_no_op`,
  `apply_changes_removed_for_an_untracked_path_is_a_no_op`).

  **`Reload`/`Conflict` never change membership, on purpose.** Both mean
  "this already-indexed file's content changed," not "a file
  appeared/disappeared" — the file's presence in the index is unaffected
  either way. `Reload` re-inserting an already-present path into the
  `HashSet` is a correct no-op (`insert` returns `false` for an existing
  member), which `apply_changes_reload_and_conflict_never_change_
  membership` checks explicitly rather than assuming.

  **What was deliberately not built: excludes integration and multi-root
  composition.** `Workspace::excludes()` (deliverable 1, `.lux-project`'s
  exclusion globs) isn't threaded into `WorkspaceIndex::build`'s scan —
  those globs are still opaque strings with no matching engine attached to
  them anywhere in the codebase (a gap flagged explicitly in deliverable
  1's own receipt, unchanged since). Wiring that through would mean
  picking how `WorkspaceIndex` learns about a `Workspace`, which is a real
  design decision belonging to whoever actually connects these pieces, not
  something to bolt on speculatively here. Likewise, `WorkspaceIndex` is
  single-root by design (matching `FileWatcher::watch` and
  `FileTree::open`, both also per-root) — a multi-root workspace composes
  N indices at the caller level, the same pattern already established for
  watchers and file trees.

  **Actual verification.** `cargo test -p lux-workspace index::`: 10/10
  passed, 0 ignored, on the first run — no iteration needed to reach
  green. Re-ran 3× consecutively afterward, all clean, to rule out
  flakiness in the real-watcher end-to-end test specifically (the same
  precaution the watcher deliverable's own receipt took for its real-
  FSEvents tests). Coverage: a full baseline scan indexing every real file
  under a root; directories themselves never indexed; `.gitignore`
  respected in the initial scan; `Created` adding a real file;
  `Created` on a directory being a no-op; `Removed` dropping an indexed
  file; `Removed` on an untracked path being a no-op; `Reload`/`Conflict`
  never touching membership; a mixed batch (one add, one remove) applied
  in a single call; and the real `FileWatcher`-fed end-to-end case. Full
  repo gate also run and green: `cargo run -p xtask` — `cargo fmt --all
  --check`; `cargo clippy --workspace --all-targets -- -D warnings` (zero
  warnings); `cargo test --workspace` (all crates, no regressions —
  `lux-workspace` now at 41 tests total: 21 `project::` + 10 `watcher::`
  from the two prior loops + 10 new `index::`; `lux-search` unchanged at
  32); `cargo deny check` (advisories/bans/licenses/sources all `ok`, no
  new `deny.toml` entries — `ignore` is already vetted, this is its
  second consumer in the workspace, not a new dependency); `scripts/
  check-limits.sh` (`index.rs` at 333 lines, under the 600 cap;
  `WorkspaceIndex` — its largest struct — has 2 fields, well under 25);
  `scripts/check-core-isolation.sh` doesn't apply (only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only.

  **What's out of scope and untouched:** sessions/recovery and huge/binary
  file handling (Phase 4 deliverables 7-8); wiring `WorkspaceIndex` into
  Goto Anything, Find in Files, or the `lux` binary; threading
  `Workspace::excludes()` into the index's scan; multi-root composition
  (a caller-level concern, one index per root); Windows/Linux verification
  (no non-macOS dev environment available).
