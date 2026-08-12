# Receipt: Stage/unstage/commit/amend (Phase 6)

- Phase: 06
- Crate(s): lux-git
- Goal: The "Stage/unstage/commit/amend" deliverable of Phase 6 ("Git
  integrado con `gix`") from `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-git staging:: commit::` (over temporary
  repositories; run as two separate `cargo test` invocations since `cargo
  test` only accepts one `TESTNAME` filter at a time — `cargo test -p
  lux-git staging::` then `cargo test -p lux-git commit::`)
- Iterations: 2 / 10
- Result: passed
- Opened: 2026-08-11
- Closed: 2026-08-11
- Notes:

  **Scope.** The master prompt's fuller Phase 6 wording is "stage y
  unstage por fichero **y por hunk**." This loop's own goal title —
  "Stage/unstage/commit/amend" — omits "por hunk," so per-hunk staging
  (building a blob from a subset of a file's hunks, then indexing that
  partial content) is deliberately out of scope: only whole-file
  stage/unstage is built. `docs/STATUS.md`'s "Stage/unstage/commit/amend"
  row stays `not-started`, the same convention as every scope-noted row in
  this document: real, tested write operations, nothing wired into
  `lux-ui` or the running `lux` binary yet.

  **Two new modules, `staging` and `commit`, matching the gate's two test
  namespaces.** `staging` owns `.git/index` reads and writes; `commit`
  depends on it (via a `pub(crate)` `open_or_create_index` helper) to read
  the currently staged content when building a tree — one index-access
  path, not two.

  **`gix::status()` considered and rejected again, same call as the
  previous loop.** `gix` has a full `status()` platform that could, in
  principle, drive staging decisions too, but it's read-oriented (working
  tree vs index vs HEAD comparison) and heavier than this deliverable
  needs. Staging itself uses `gix_index`'s lower-level entry API directly
  (`open_index`/`dangerously_push_entry`/`remove_entries`/`sort_entries`/
  `write`) — the right layer for "write this file's current content into
  the index," which is what stage/unstage actually are.

  **Index entries carry a real stat and a real executable bit**, not
  placeholders: `gix_index::entry::Stat::from_fs` (built from
  `gix_index::fs::Metadata::from_path_no_follow`, which uses `rustix`'s
  `lstat` on Unix — the same primitive `git` itself relies on for stat
  comparisons) and `Metadata::is_executable()` decide `Mode::FILE` vs
  `Mode::FILE_EXECUTABLE`. Skipping the executable bit would have been the
  lazier path, but it's a real, common data-loss-adjacent bug (`git`
  losing a script's executable bit on stage/commit is a familiar
  complaint), and detecting it correctly was one `gix`-provided call away
  — not scope creep, just not cutting a corner that was this cheap to keep.

  **`commit`'s tree is built from `HEAD`'s tree plus the index, not from
  the index alone.** `gix::Tree::edit()` (the `tree-editor` feature)
  starts from `HEAD`'s current tree (or an empty one, via
  `head_tree_id_or_empty`, on an unborn `HEAD`) and upserts every index
  entry on top. This matters because `lux-git`'s index is real but
  intentionally minimal — nothing in this crate populates it from a full
  worktree scan, only `staging::stage` adds entries one file at a time —
  so building the tree from the index in isolation would silently drop
  every file that's actually committed at `HEAD` but hasn't been
  re-staged in the current session. Starting from `HEAD`'s tree is what
  makes `second_commit_has_first_as_parent`'s assertion true (files from
  the first commit are still present after a second commit that only
  staged one new file).

  **The one real design finding this loop surfaced (not a `gix` gap, an
  API-fit issue caught by a failing test, not by reading docs first):**
  `amend`'s first implementation reused `commit()`'s call to
  `Repository::commit_as()`, and both amend tests failed with
  `ReferenceOutOfDate`/`MustNotExist` ref-transaction errors. Reading
  `commit_as`'s source (`gix-0.86.0/src/repository/object.rs`) showed why:
  it treats `parents.first()` as the ref's *expected current value* before
  the update — correct for an ordinary successor commit, where `HEAD`
  equals its own new parent, but wrong for amend, where the new commit's
  parents are its *predecessor's* parents, while `HEAD` currently holds
  the predecessor itself. Fixed by not using `commit_as` for amend at all:
  write the `gix_object::Commit` directly with `repo.write_object()`, then
  perform the `HEAD` ref update by hand via `repo.edit_reference()` with
  `expected: PreviousValue::MustExistAndMatch(Target::Object(original_id))`
  — the commit being replaced, not its parent. This is worth a durable
  note because it's a real API-shape lesson, not a one-off typo: any
  future `lux-git` operation that rewrites history in place (interactive
  rebase, cherry-pick with `--no-commit` semantics, etc.) will hit the
  same mismatch between `commit_as`'s "successor commit" assumption and
  "replace the current commit" semantics, and should reach for
  `write_object` + manual `edit_reference` from the start rather than
  rediscovering this by a failing ref-transaction error.

  **Other fixes along the way, none requiring a second loop:**
  `entry::Flags::default()` doesn't exist (bitflags 2.x doesn't derive
  `Default`) — `Flags::empty()` instead. `Head::try_peel_to_id_in_place`
  is deprecated in `gix` 0.86.0 in favor of `try_peel_to_id` — one-line
  fix, no behavior change. `BStr::to_str()` needed `gix::bstr::ByteSlice`
  in scope (used once, in the first `amend` draft, then removed once
  `amend` switched to `.into()` for the message conversion instead).

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git
  staging::` — 5 tests, 0 failures, 0 ignored (staging a new file, staging
  twice replaces rather than duplicates the entry and picks up the new
  content, unstage removes the entry without touching the file, unstaging
  something never staged is a no-op not an error, staged state survives a
  fresh `Repository`/index read from disk). `cargo test -p lux-git
  commit::` — 6 tests, 0 failures, 0 ignored (a first commit has no
  parent and contains what was staged, a second commit has the first as
  parent and still contains the first commit's files, a commit only
  includes staged files — not every file in the working copy, amend
  replaces the tree and message while keeping the original parents and
  author but updating the committer, amend without a new message keeps
  the original, amending an unborn `HEAD` fails). All against real
  temporary repositories built with `gix`'s own object-writing API — no
  `git` binary anywhere in the fixtures or the implementation. Also clean:
  `cargo clippy -p lux-git --all-targets -- -D warnings`, `cargo fmt
  --check -p lux-git`, `./scripts/check-limits.sh` (`commit.rs` 370
  lines, `staging.rs` 201 lines, both under the 600-line cap), `cargo test
  -p lux-git` (all 34 tests across `gutter`, `diff_panel`, `staging`, and
  `commit` still pass together), and `cargo build --workspace`.
