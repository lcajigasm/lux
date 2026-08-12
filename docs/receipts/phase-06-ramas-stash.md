# Receipt: Ramas, stash (Phase 6)

- Phase: 06
- Crate(s): lux-git
- Goal: The "Ramas, stash" deliverable of Phase 6 ("Git integrado con
  `gix`") from `LUX_PROMPT_MAESTRO.md` is complete and its gate passes
  with no failures and no ignored tests.
- Verify: `cargo test -p lux-git branches:: stash::` (over temporary
  repositories; run as two separate `cargo test` invocations, since `cargo
  test` only accepts one `TESTNAME` filter at a time — `cargo test -p
  lux-git branches::` then `cargo test -p lux-git stash::`)
- Iterations: 3 / 8
- Result: passed
- Opened: 2026-08-11
- Closed: 2026-08-11
- Notes:

  **Scope.** Two new modules, matching the gate's two test namespaces and
  the master prompt's own wording ("cambio y creación de ramas; stash").
  `docs/STATUS.md`'s "Branches & stash" row stays `not-started`, the same
  convention as every scope-noted row in this document: real, tested
  write operations, nothing wired into `lux-ui` or the running `lux`
  binary yet.

  **Branch switching had to be a real checkout, not a `HEAD` pointer
  move, or it would violate golden rule 1 ("nothing simulated").** A
  "switch branches" that only updates `HEAD` symbolically and leaves
  stale files on disk isn't a working switch — it's a broken one wearing
  the name. So `branches::switch` genuinely materializes the target
  tree: deletes paths the old tree had that the new one doesn't, writes
  every blob the new tree specifies (creating directories as needed, with
  the recorded executable bit), and rebuilds the index to match. That
  materializer, `checkout_tree`, is `pub(crate)` and shared with
  `stash::save`/`pop` — both need the identical "make the working copy
  match this tree" operation, so it's written once.

  **`gix_worktree_state`'s full checkout API was evaluated and
  deliberately not used — the ADR 0010 "check before assuming a gap"
  discipline cuts both ways.** `gix` ships a real checkout function
  (`Repository::blame_file()`'s sibling in spirit: `gix_worktree_state::
  checkout`, behind the `worktree-mutation` feature) built for clone-time
  and `git checkout --force` parity, including `.gitattributes`-driven
  text-conversion filters. Its `Options` require a filesystem-capability
  probe (`gix_fs::Capabilities`), an attributes stack, and a real
  `gix_filter::Pipeline` — real machinery, but built for a bigger job
  (attribute-aware filtering) than "switch and write files" needs here.
  Given the 8-iteration budget, wiring that correctly would have been a
  separate task in itself; `checkout_tree` instead reuses primitives
  already proven in this crate (tree traversal from `diff_panel`, blob
  reading from `gutter`/`commit`, index writing from `staging`) — smaller
  surface, same crate's own established patterns, no filter/attribute
  semantics (which live in `lux-config::gitattributes`, a separate Phase
  5 piece not yet integrated with checkout). This scope line is worth
  revisiting explicitly if a later Phase 6 or 10 loop needs
  attribute-aware checkout (EOL normalization on checkout, for instance).

  **Stash's design changed mid-loop, and the working version doesn't use
  `git`'s own on-disk mechanism.** First attempt: store the stash stack
  in `refs/stash`'s reflog, the way real `git stash list` works — read
  the log with `Reference::log_iter().rev()`, chain `previous_oid`/
  `new_oid` to reconstruct the stack. Two problems surfaced by the tests,
  not by reading docs first:
  1. `Repository::reference()`'s convenience wrapper hardcodes
     `force_create_reflog: false`, which is correct for a branch (whose
     reflog existence follows `core.logAllRefUpdates`) but means nothing
     ever creates a reflog for an arbitrary ref like `refs/stash` —
     `list()` read back zero entries because the log file never existed.
     Fixed by calling `Repository::edit_reference()` directly with
     `force_create_reflog: true`.
  2. Once the reflog existed, a `pop()`-triggered ref update *also*
     appended a reflog line (since `RefLog::AndReference` logs every
     update, not just pushes), and that line's shape broke the
     chain-walk: its `new_oid` coincidentally matched the post-pop `refs/
     stash` value, so `list()` misread it as a legitimate stash entry
     and returned 3 entries instead of 1 after a single pop.

     Rather than teach `list()` to distinguish "push" lines from "pop"
     lines by sniffing the log message (fragile — a user-supplied stash
     message could collide with the sentinel), the design changed: the
     stack is threaded through each stash commit's own **second parent**
     instead (`save` writes `parents: [HEAD, previous top-of-stack]`;
     `list`/`pop` walk `parents.get(1)` down to the bottom). This sidesteps
     reflog interpretation entirely — `refs/stash`'s reflog still exists
     (for basic auditability) but nothing reads it anymore. The module's
     own doc comment states plainly that this means a repository stashed
     by `lux-git` isn't guaranteed to also work with the real `git stash`
     CLI on the same checkout: the stack representation is `lux`'s own,
     not `git`'s reflog convention, and the stash commit carries one tree
     (working-tree content only) rather than `git`'s separate
     index/worktree distinction. This is a real, working stash — tested
     multi-entry push/pop, not a stub — just not byte-for-byte compatible
     with upstream `git stash`'s storage format.

  **One `gix` semantics finding worth stating plainly, caught by a
  failing test:** `PreviousValue::MustNotExist` on a ref *update* (not a
  deletion) only rejects the write if the existing ref's target actually
  *differs* from the new value — recreating a branch with the exact same
  target is treated as an idempotent no-op success, per `gix-ref`'s own
  file-transaction source. `branches::create`'s first test asserted
  create-twice-with-the-same-commit fails; that assertion was simply
  wrong about `gix`'s real semantics. Fixed by testing the actual failure
  mode (creating an existing branch name pointing at a *different*
  commit) and adding a second test (`create_with_the_same_target_twice_
  is_a_no_op`) that pins the idempotent case down as documented,
  intentional behavior instead of leaving it as an undocumented surprise
  for the next loop to rediscover.

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git
  branches::` — 6 tests, 0 failures, 0 ignored (create points at the
  given commit and shows up in `list`, create fails when the branch
  already points elsewhere, create with an identical target is a no-op,
  switch updates `HEAD` and materializes the target tree — deleting files
  the new branch doesn't have and writing files it does, switch correctly
  rewrites a file whose content differs between branches, the index after
  switch matches the target tree). `cargo test -p lux-git stash::` — 5
  tests, 0 failures, 0 ignored (save resets the working tree to `HEAD`
  and pop brings the change back, saving with no changes is an error,
  popping with no stash is an error, `list` shows multiple stashes
  newest-first and `pop` consumes from the top with the stack correctly
  shrinking, stashing a deleted tracked file and popping restores the
  deletion). All against real temporary repositories built with `gix`'s
  own object-writing API — no `git` binary anywhere. Also clean: `cargo
  clippy -p lux-git --all-targets -- -D warnings`, `cargo fmt --check -p
  lux-git`, `./scripts/check-limits.sh` (`branches.rs` 408 lines,
  `stash.rs` 410 lines, both under the 600-line cap), `cargo test -p
  lux-git` (all 53 tests across every `lux-git` module still pass
  together), and `cargo build --workspace`.
