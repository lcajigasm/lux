# Receipt: Resolución de conflictos de merge (Phase 6)

- Phase: 06
- Crate(s): lux-git
- Goal: The "Resolución de conflictos de merge" deliverable of Phase 6
  ("Git integrado con `gix`") from `LUX_PROMPT_MAESTRO.md` is complete
  and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-git merge_conflicts::` (over a temporary
  repository with an induced conflict)
- Iterations: 1 / 10
- Result: passed
- Opened: 2026-08-11
- Closed: 2026-08-11
- Notes:

  **Scope.** The master prompt's wording: "resolución de conflictos de
  merge con acciones 'Accept Current / Incoming / Both' **sobre el
  editor**." The "sobre el editor" part is explicitly UI-layer work —
  `lux-ui` doesn't exist yet as a merge-conflict surface, and golden rule
  2 keeps `lux-git` free of any UI dependency. This loop builds the
  backend primitives such editor actions would call — a real merge that
  surfaces content conflicts, and a way to resolve each one with an
  ours/theirs/both choice — not the editor commands themselves.
  `docs/STATUS.md`'s "Merge conflict resolution" row stays `not-started`,
  the same convention as every scope-noted row in this document.

  **The headline finding, again: check `gix` before assuming a gap pays
  off.** `gix` has a real, mature merge engine behind the `merge` feature
  (`dep:gix-merge`) — `Repository::merge_commits()` performs an actual
  three-way merge with automatic merge-base detection and returns a
  structured `Conflict` list (`gix_merge::tree::Conflict`) with base/ours/
  theirs index entries per conflicted path. This is the same ADR
  0010 discipline that already avoided reimplementing line attribution
  for blame two loops ago: checking `gix`'s feature list first turned
  what could have been a from-scratch three-way-merge implementation
  (a genuinely hard algorithm — merge-base detection, tree diffing,
  text-conflict marker generation) into a wrapper around real, tested
  upstream code. `merge_conflicts::merge()` is essentially: call
  `Repository::merge_commits()`, write the resulting tree, and translate
  `gix-merge`'s `Conflict` list into this crate's own simpler
  `MergeConflict { path, base, ours, theirs }` shape.

  **The merge result is materialized for real, reusing the same
  `checkout_tree` primitive branch-switching and stash already share.**
  `merge()` doesn't just return data — it writes the merged tree (with
  real conflict markers at unresolved paths, exactly what `git merge`
  itself writes) onto the working directory and index. This was a
  deliberate consistency decision: three earlier Phase 6 loops
  (branches, stash) already established "make the working copy match
  this tree" as one shared operation; a fourth ad-hoc way to write files
  for merges would have been unnecessary duplication for no benefit.

  **Scope is deliberately narrower than everything `gix-merge` reports,
  stated explicitly rather than silently dropped.** `gix-merge` also
  detects rename conflicts and delete/modify conflicts (one side removed
  a path the other changed) — real, structured outputs, not gaps in
  `gix` itself. This loop only turns *content conflicts* (both sides
  modified, or both sides independently added, the same path — the
  common case an "Accept Current/Incoming/Both" UI targets) into a
  resolvable `MergeConflict`. Hitting a rename or delete/modify conflict
  returns `MergeConflictError::UnsupportedConflictShape` with the path
  and reason, rather than silently mapping it into the wrong shape or
  dropping it from the conflict list — a caller always knows when it hit
  something this module doesn't yet resolve, instead of getting a
  plausible-looking but wrong result. This is a real, statable scope
  line (matching every other narrowing decision across this phase's
  receipts), not a gap discovered by accident.

  **"Both" resolution order.** `Resolution::Both` concatenates ours then
  theirs, verbatim, no deduplication or separator beyond what the file
  content already has. This mirrors what most editors' "Accept Both
  Changes" actually does (keep both hunks, current side first) rather
  than attempting any content-aware merging beyond what `gix-merge`
  already performed.

  **No struggle worth narrating.** All 7 tests passed on the first real
  compile, after two mechanical fixes caught immediately by `cargo
  check` (an unused `Path` import, and a missing `mut` on the merge
  outcome binding needed because writing the tree editor takes `&mut
  self`) — no logic bugs surfaced by a failing test this time.

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git
  merge_conflicts::` — 7 tests, 0 failures, 0 ignored, all against a real
  temporary repository with a genuinely **induced** conflict (`main` and
  a `feature` branch each independently edit the same line of the same
  file, diverging from a shared base commit — not a synthetic conflict
  object constructed by hand):
  - a merge with no real conflicts reports none, and the non-conflicting
    change is already merged onto disk;
  - the induced conflict is detected with exactly one entry, correct
    `ours`/`theirs` blob content matching each branch's actual edit, and
    the working copy genuinely contains `<<<<<<<` conflict markers after
    `merge()` — not asserted from the API's return value alone;
  - `Resolution::Ours`/`Theirs`/`Both` each produce the correct on-disk
    content;
  - `complete()` after resolving writes a real two-parent commit (parents
    are exactly `[our_commit, their_commit]`) whose tree reflects the
    chosen resolution;
  - resolving and completing leaves the file in the resolved state, not
    still conflict-marked.

  Also clean: `cargo clippy -p lux-git --all-targets -- -D warnings`,
  `cargo fmt --check -p lux-git`, `./scripts/check-limits.sh`
  (`merge_conflicts.rs` 429 lines, under the 600-line cap), `cargo test
  -p lux-git` (all 60 tests across every `lux-git` module still pass
  together — `gutter`, `diff_panel`, `staging`, `commit`, `history`,
  `blame`, `branches`, `stash`, `merge_conflicts`), and `cargo build
  --workspace`.

  **Phase 6 status.** This closes the last row of Phase 6's capability
  table (`docs/STATUS.md`'s "Git integration (Phase 6, v0.5.0)"
  section) — all six deliverables now have an individual `Result: passed`
  receipt under `docs/receipts/phase-06-*.md`. A dedicated close-out loop
  (mirroring `docs/receipts/phase-05-release-notes.md`'s role for Phase
  5) is a separate, explicit piece of work, not implied by this receipt.
