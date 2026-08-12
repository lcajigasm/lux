# Phase 6 release notes — Git integrado con `gix`

Commits from `f48190c` (end of Phase 5) to `HEAD` (7 commits, plus this
close-out commit).

## Highlights

- **`lux-git`** (new crate, ten modules), entirely in-process via `gix`
  0.86.0 — no shell-out to the `git` binary, anywhere, per ADR 0010:
  - **`gutter`**: change indicators and hunk navigation. Diffs a file's
    working-copy content against its `HEAD` blob using `gix`'s own
    histogram-diff engine, not a hand-rolled one. `HunkNavigator` wraps
    around at either end for next/previous-change commands.
  - **`diff_panel` + `diff_view`**: the changes tree (which files
    changed, grouped by folder, gitignore-aware) and both diff renderings
    — side-by-side and inline (unified) — built on the same
    `gutter::diff_hunks` engine, one diff computation per pair of texts,
    not per view.
  - **`staging`**: real index (`.git/index`) mutation — whole-file and
    per-hunk `stage`/`unstage`, with real stat info and executable bits
    read from actual file metadata, not assumed.
  - **`commit`**: real commit objects with multi-line messages, and
    `amend` that preserves the original author and parents while updating
    the committer, the same default `git commit --amend` has.
  - **`history` + `blame`**: real revision walks (`gix`'s own, sorted
    newest-first) for repository and per-file history; blame wraps `gix`'s
    first-party blame algorithm (the `gix-blame` crate) rather than
    reimplementing line attribution.
  - **`branches` + `stash`**: branch creation and switching as a real
    forced checkout (materializes the target tree onto disk and index, not
    just a `HEAD` pointer move); a functional stash using the standard
    `refs/stash` name with `lux`'s own second-parent-threaded stack.
  - **`merge_conflicts`**: a real three-way merge via `gix`'s own merge
    engine (`gix-merge`), surfacing content conflicts with `Ours`/
    `Theirs`/`Both` resolution — the primitives an "Accept Current /
    Incoming / Both" editor action would call.
  - **`status`**: current branch (named/detached/unborn) and a
    dirty/clean + changed-file summary, reusing `diff_panel`'s own
    change-detection rather than a second path.
- **`gix`'s real maturity checked, not assumed, three separate times** —
  the ADR 0010 discipline paying off repeatedly, not a one-off: `gix-blame`
  for line attribution (blame), `gix-merge` for three-way merging (merge
  conflicts), and `gix_diff::blob` (the same histogram-plus-slider-
  heuristics engine `gix` itself uses) for line diffing (gutter). None of
  these were reimplemented from scratch; each check turned a
  potentially-large from-scratch algorithm into a focused wrapper around
  real, tested upstream code.
- **Verified against a real, large repository** — Phase 6's own
  acceptance criteria requirement, exercised for the first time in the
  phase's closing loop: a shallow clone of `rust-lang/rust` (~62,000
  files, a 71,164-line real source file). Every `lux-git` operation ran
  against it end-to-end (status, changed-files walk, gutter diff, both
  diff views, per-hunk staging, a real commit, history walk, branch
  create/switch materializing the full tree, stash save/pop, a real
  induced three-way merge with conflict resolution), surfacing two real
  bugs — see Findings below — that no small, hand-built temp-repo fixture
  could have structurally reached.

## Findings during this phase, not just modules landing on schedule

- **A real bug in `changed_files`/`status`, found only by testing against
  a repository with real history**: a tracked file that also matches a
  `.gitignore` rule added *after* it was committed (rust-lang/rust has 12
  such files — a completely ordinary real-world pattern, not project
  idiosyncrasy) was reported as deleted, because the gitignore-aware
  directory walk correctly skips it and the code treated "the walk didn't
  find this tracked path" as "it's gone." Real `git status` never lets
  `.gitignore` hide an already-tracked path. Fixed by checking every
  tracked path directly against disk, independent of the gitignore-aware
  walk, which is now used only to discover genuinely untracked files.
- **A real bug in `stash::save`**: a file that was `git add`-ed but never
  committed was invisible to it — it only scanned `HEAD`'s tree, not the
  index — so stashing right after staging a new file failed with
  "nothing to stash." Real `git stash`, even without `-u`, does capture
  staged-but-uncommitted files. Fixed by unioning `HEAD`'s tree with the
  current index before building the snapshot.
- **A real, reproducible `gix` limitation found and *not* worked around,
  per ADR 0010's explicit instruction**: `blame::blame_by_commit` fails
  on shallow clones once its history walk needs to cross the shallow
  boundary — `gix-blame` 0.16.0 reads the boundary commit's raw `parent`
  field and tries to fetch an object that was never fetched by design,
  where real `git blame` consults `.git/shallow` and treats that commit
  as parentless. Confirmed directly (`git cat-file -t` on the missing
  parent fails identically — this is `git`'s intended shallow-clone
  behavior, not corruption). Narrow in practice: a normal, non-shallow
  clone — the ordinary case for a repository opened in an editor — has
  every object blame might need.
- **A real `gix` ref-transaction semantics finding, caught by a failing
  test, not documentation**: `PreviousValue::MustNotExist` on a ref
  *update* only rejects the write if the existing ref's target actually
  *differs* from the new value — recreating a branch with the identical
  target is treated as an idempotent no-op success, per `gix-ref`'s own
  file-transaction source. `branches::create`'s first test assumed the
  opposite and was simply wrong about `gix`'s real semantics; fixed by
  testing the actual failure mode (moving an existing branch to a
  *different* commit) and adding a second test pinning the idempotent
  case down as documented, intentional behavior.
- **`Repository::commit_as()` doesn't fit `amend`'s ref-update shape**:
  it assumes it's creating an ordinary successor commit, where `HEAD`
  currently equals the new commit's first parent, and rejects the
  update otherwise — correct for `commit()`, structurally wrong for
  `amend()`, whose new commit's parents are its *predecessor's* parents
  while `HEAD` currently holds the predecessor itself. `amend` writes
  the commit object directly and performs the `HEAD` ref update by hand
  instead, expecting the commit being replaced as the ref's current
  value.
- **`gix_worktree_state`'s full checkout API was evaluated and
  deliberately not used** for branch switching: it's built for
  clone-time/`git checkout --force` parity with `.gitattributes`-driven
  text-conversion filters (filesystem-capability probing, an attributes
  stack, a real `gix_filter::Pipeline`) — more machinery than "switch and
  write files" needs. `checkout_tree` instead reuses primitives already
  proven elsewhere in this crate (tree traversal, blob reading, index
  writing).

## Known gaps (honest, documented, not silently deferred)

- **No Phase 6 capability is wired into `crates/lux-ui` or the running
  `lux` binary** — identical to every phase's own gap since Phase 3.
  `lux-ui` doesn't consume `lux-git` at all yet: no gutter rendering, no
  changes panel, no diff view, no stage/commit UI, no history/blame
  panel, no branch switcher, no stash UI, and critically, **no "Accept
  Current / Incoming / Both" editor commands** — the specific UI actions
  named in the master prompt's merge-conflict-resolution bullet. Building
  that integration is Phase 6 UI-wiring work, a different and much larger
  scope than the backend primitives this phase delivers; every capability
  above is real, tested, and correct at the `lux-git` crate level, none
  of it is yet something an actual user of the running app has.
- **Per-hunk staging exists; per-hunk stash does not** — `stash` always
  snapshots every tracked-or-staged file's current on-disk content,
  matching plain `git stash`'s default (no `-u`, no partial-hunk stash).
- **Rename and delete/modify merge conflicts are real, detected
  `gix-merge` outputs this crate doesn't yet turn into a resolvable
  `MergeConflict`** — `merge()` reports them via
  `MergeConflictError::UnsupportedConflictShape` rather than
  misrepresenting them, but resolving them isn't built.
- **The `gix-blame`/shallow-clone limitation** documented above.
- Windows and Linux remain `not-started` for the whole phase — no
  non-macOS dev environment available, same constraint as every phase
  before it.
- The large-repository verification pass (rust-lang/rust) is a manual
  check, not part of the automated `cargo test` suite — a 459 MB external
  clone doesn't belong in routine CI, matching how every other phase's
  "verified on..." line has always described a manual pass, not a
  promise that the exact commands run in CI.

## Commits

```
c462490 feat(lux-git): add status summary, inline diff, and per-hunk staging
d05ddd9 feat(lux-git): add merge conflict detection and resolution
7f144f0 feat(lux-git): add branch creation/switching and stash
573e80f feat(lux-git): add repository/file history and blame
662793a feat(lux-git): add stage/unstage, commit, and amend
b3e1238 feat(lux-git): add changes tree panel and side-by-side diff
a617417 feat(lux-git): add gutter change indicators and hunk navigation
```
