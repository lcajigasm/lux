# Receipt: Panel de cambios en árbol + diff lado a lado (Phase 6)

- Phase: 06
- Crate(s): lux-git
- Goal: The "Panel de cambios en árbol + diff lado a lado" deliverable of
  Phase 6 ("Git integrado con `gix`") from `LUX_PROMPT_MAESTRO.md` is
  complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-git diff_panel::`
- Iterations: 2 / 10
- Result: passed
- Opened: 2026-08-11
- Closed: 2026-08-11
- Notes:

  **Scope.** The full Phase 6 bullet also mentions "panel de cambios ...
  agrupado por carpeta; diff lado a lado **y en línea**." This loop's own
  goal title narrows it to "árbol + diff lado a lado" — inline diff
  rendering is deliberately out of scope here (a separate, later piece).
  Also out of scope: staging/unstaging (a distinct Phase 6 bullet,
  "stage y unstage por fichero y por hunk"), renames/copies detection,
  and any wiring into `lux-ui`'s actual panels — none of that is touched.
  `docs/STATUS.md`'s "Diff panel (side-by-side/inline)" row stays
  `not-started`, same convention as every scope-noted row in this
  document: real, tested library capability, nothing wired into the
  running `lux` binary yet.

  **Reused, not rebuilt, this crate's own diff engine.** `side_by_side`
  calls `gutter::diff_hunks` (from the previous Phase 6 loop) rather than
  computing its own diff — one diff engine in `lux-git`, not two.

  **Changed-file detection: `gix` for the `HEAD` tree, the `ignore` crate
  for the working-directory walk — not `gix::status`.** `gix` does have a
  full `status()` platform (`Repository::status`, feature `status`) that
  covers this same ground (tree/index/worktree comparison, untracked
  files, renames, submodules), but its API surface is built around
  progress-reporting generics, a real on-disk index, thread-limit
  configuration, and submodule-status policy — correct for a full `git
  status` replacement, but far more than this deliverable's "list what
  changed, grouped by folder" actually needs, and every one of those knobs
  is a place this loop could have gotten stuck without moving the needle
  on the goal. Instead: read `HEAD`'s tree recursively via `Tree::
  traverse().breadthfirst.files()`, walk the working directory with the
  `ignore` crate (already a workspace dependency — `lux-search` and
  `lux-workspace` both use it since Phase 4 — for the exact same
  gitignore-matching job), and diff the two path sets directly. This is a
  deliberate scope choice, not an oversight: it produces correct
  Added/Modified/Deleted classification against a real repository
  (verified below) without pulling in machinery this deliverable doesn't
  ask for. Revisit with `gix::status` specifically if a later Phase 6 loop
  needs something this approach can't do (renames, submodule status,
  index-aware staged-vs-unstaged distinction).

  **`side_by_side` alignment.** Every line of both texts becomes a row;
  context (unchanged) lines pair 1:1 between hunks, and within a hunk the
  shorter side is blank-padded so a pure insertion or deletion still lines
  up visually against the other side — the same shape a split diff view
  (GitHub's, or any editor's) renders. Built as a plain function
  (`push_context`) taking explicit `&mut` cursor arguments rather than a
  capturing closure — the closure version doesn't borrow-check (a closure
  that mutably captures `before_pos`/`after_pos` while the enclosing loop
  also reads them for the `Changed`-row indices conflicts under Rust's
  borrow rules), and threading the two `usize` cursors through explicitly
  is barely more code for a plain function that does compile.

  **What tripped the first pass, fixed within the same iteration:**

  1. The `side_by_side` closure-borrow issue above (a compile error, not a
     wrong-answer bug) — fixed by converting `push_context` from a closure
     to a top-level function.
  2. `deprecated: work_dir()` — `gix` 0.86.0 renamed it to `workdir()`;
     one-line fix, no behavior change.
  3. `unused_imports: Path` — `std::path::Path` was only used inside the
     `#[cfg(test)] mod against_real_repo` submodule, not the crate's
     non-test build; moved the import into that submodule instead of the
     file's top level.
  4. One test-fixture bug, not an implementation bug:
     `against_real_repo::detects_added_modified_and_deleted_files`
     constructed a tree with entries in insertion order instead of
     sorted-by-filename order, and `gix_object::tree::write` asserts
     entries are pre-sorted before serializing (`"entries for
     serialization must be sorted by filename"`) rather than silently
     sorting for the caller — a real correctness rule (tree object
     encoding is order-sensitive in Git's on-disk format), not a gix
     quirk to work around. Fixed by sorting the test fixture's `Vec<Entry>`
     before building the `Tree`. No durable correction worth promoting to
     `CLAUDE.md` — this is the second receipt in a row to build a `gix`
     tree-from-entries fixture by hand; if a third Phase 6 loop needs the
     same pattern, that's the trigger to extract a small shared test
     helper (not yet, per YAGNI — two call sites don't justify it).

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git
  diff_panel::` — 10 tests, 0 failures, 0 ignored (7 pure-function tests
  covering folder grouping — root-level, nested, deeply-nested — and
  side-by-side alignment — identical text, a modified line, padding on
  pure insertion, padding on pure deletion; 3 integration tests against
  real temporary `gix` repositories). Also clean: `cargo clippy -p lux-git
  --all-targets -- -D warnings`, `cargo fmt --check -p lux-git`,
  `./scripts/check-limits.sh` (506 lines, under the 600-line cap), `cargo
  test -p lux-git` (all 23 tests across both `gutter` and `diff_panel`
  still pass together), and `cargo build --workspace`.
