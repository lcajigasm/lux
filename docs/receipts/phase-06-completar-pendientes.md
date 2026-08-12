# Receipt: Completar pendientes de la Fase 6 (status, diff en línea, stage por hunk, verificación a gran escala)

A receipt records one loop's execution — how many iterations it took, what
failed along the way, what fixed it, and whether it got stuck. It is not an
ADR: an ADR explains a design decision that survives forever, a receipt is
the log of one concrete run, mainly so the next session knows exactly where
the work was left without you having to explain it.

- Phase: 06
- Crate(s): lux-git
- Goal: Close the real gaps a `/close-phase 6` audit found against the full
  Phase 6 entregable list and its own acceptance criteria: a
  repository-status summary; the inline half of "diff lado a lado y en
  línea"; per-hunk staging ("stage... y por hunk"); and — the acceptance
  criteria's own explicit requirement, not yet exercised by any of the six
  prior Phase 6 receipts — running the read/write operations against a
  real, large repository.
- Verify: `cargo test -p lux-git` (74 tests) plus the full workspace gate
  (`cargo test --workspace && cargo clippy --workspace -- -D warnings &&
  cargo deny check && ./scripts/check-limits.sh && ./scripts/check-core-isolation.sh`),
  plus a manual verification pass against a real, large repository
  (rust-lang/rust, shallow-cloned) — not part of the automated suite.
- Iterations: 1 / — (open-ended "finish what's pending" goal, not a fixed
  iteration budget)
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** A `/close-phase 6` run immediately before this loop found the
  phase couldn't close: 6 of 11 semicolon-separated entregables in the
  master prompt's Phase 6 bullet were fully covered by existing receipts,
  1 (repository status/branch/change-count summary) had no code or receipt
  at all, 3 were explicitly narrowed in scope by their own receipts (inline
  diff, per-hunk staging, and the "Accept Current/Incoming/Both" editor
  actions), and the phase's acceptance criteria's explicit large-repository
  requirement had never been exercised. This loop closes the first three
  gaps and runs the fourth. The fourth (editor UI actions) stays out of
  scope deliberately: it requires `lux-ui` to consume `lux-git` at all,
  which doesn't exist yet, and building that integration is a different,
  much larger scope than "finish `lux-git`'s pending backend work" — see
  the closing note below.

  **New: `lux-git::status`.** `status()` reports the current branch (a
  `BranchState` covering named/detached/unborn `HEAD`, mirroring `gix::
  head::Kind` directly rather than inventing a parallel model) and a
  changed-file count, reusing `diff_panel::changed_files` for the count so
  there's one change-detection code path feeding both the changes panel
  and the status summary, not two that could disagree.

  **New: `lux-git::diff_view` (split out of `diff_panel`).** Adding
  `inline_diff` (a unified-diff view: removed lines then added lines per
  hunk, in file order, matching `git diff`'s own convention) alongside the
  existing `side_by_side` pushed `diff_panel.rs` to 669 lines, over the
  600-line cap (golden rule 4). Rather than trim comments to squeeze under
  the limit, the two diff-*rendering* functions (`side_by_side`,
  `inline_diff`, both pure transforms built on `gutter::diff_hunks`, no
  repository access) moved into a new `diff_view` module, leaving
  `diff_panel` focused on its one remaining concern — the changes-tree
  data source (`changed_files`, `group_by_folder`). This is a real,
  principled split along an existing conceptual seam the module's own doc
  comment already named ("which files changed" vs. "what does one look
  like"), not a mechanical line-count dodge.

  **New: `lux-git::staging::{unstaged_hunks, stage_hunk}`.** Per-hunk
  staging diffs a path's currently-staged-or-`HEAD` content against its
  working-copy content (via `gutter::diff_hunks`, the same engine every
  other diff in this crate uses) to produce the hunk list, then
  `stage_hunk` applies exactly one selected hunk on top of that same base
  by slicing and rejoining lines — matching `git add -p`'s "stage this
  hunk" semantics, not a whole-file replace. Verified with two
  non-adjacent edits to one file, staged one hunk at a time, each landing
  independently.

  **The large-repository verification pass — the reason this loop exists —
  found two real bugs neither the existing ~70-test suite nor careful
  reasoning had caught.** Built a throwaway verification harness (a
  separate scratch Cargo binary depending on `lux-git` by path, deleted
  before finishing — not shipped, not a cargo test, since bundling a
  459 MB external clone into routine CI is neither desirable nor what
  "verified on..." has meant in any prior Phase 6 receipt) driving every
  `lux-git` operation against a real shallow clone of `rust-lang/rust`
  (~62,000 files; the largest real file tested was 71,164 lines /
  2.68 MB). Findings:

  1. **`diff_panel::changed_files` reported 12 real, unmodified, tracked
     files as deleted** — every one matched a `.gitignore` rule added
     after it was committed (`.vscode/settings.json`,
     `src/tools/rust-analyzer/.cargo/config.toml`, etc. — a completely
     ordinary real-world pattern, not a rust-lang/rust idiosyncrasy). Root
     cause: the function discovered on-disk files via a single
     gitignore-aware directory walk and treated "the walk didn't find
     this tracked path" as "it's gone" — but `.gitignore` only ever
     governs discovery of *untracked* files in real `git`; it can never
     hide a path git already tracks. Fixed by splitting into two clearly
     separate passes: every tracked path (from `HEAD`'s tree) is now
     checked directly against disk regardless of ignore status, and the
     gitignore-aware walk is used only to discover genuinely untracked
     (candidate-`Added`) paths. Caught a false positive that a small
     synthetic test repo — which never happens to have a tracked file
     matching a *later*-added ignore rule — structurally cannot surface;
     added `a_tracked_file_that_matches_a_later_gitignore_rule_is_not_reported_as_deleted`
     as a permanent regression test reproducing the exact shape.
  2. **`stash::save` refused to stash a newly `git add`-ed, not-yet-committed
     file**, returning `NothingToStash` despite a real staged change.
     Root cause: the snapshot builder only walked `HEAD`'s tree, so a path
     that exists only in the *index* (a brand-new staged file) was
     invisible to it. Real `git stash`, even without `-u`, does capture
     staged-but-uncommitted files — only genuinely untracked-and-unstaged
     ones are excluded by default. Fixed by unioning `HEAD`'s tree paths
     with the current index's paths before building the snapshot tree.
     Added `save_includes_a_newly_staged_file_not_yet_committed` as a
     regression test.

  Both bugs share a shape worth naming for future `lux-git` work: each
  came from conflating "what a directory walk or a single tree finds"
  with "the true set of paths git cares about," which only diverges when
  a repository has accumulated real history (old commits predating a
  `.gitignore` rule, or an in-progress staged-but-uncommitted edit) — the
  exact kind of state a hand-built temp-repo fixture never has by
  construction. **Durable note for future receipts in this crate**: when
  a function's job is "enumerate paths relevant to git," reach for the
  index and `HEAD`'s tree as the source of truth for *tracked* paths, and
  treat any directory walk (gitignore-aware or not) as relevant only for
  discovering genuinely *un*tracked paths — never let a walk's absence of
  a path stand in for "git doesn't know about it."

  **One finding surfaced and deliberately left unfixed, per ADR 0010's
  explicit instruction to write up gaps rather than route around them
  silently**: `blame::blame_by_commit` fails with a `NotFound` object
  error on shallow clones once its walk needs to cross the shallow
  boundary. Confirmed the root cause directly (not just observed the
  symptom): the boundary commit's raw object bytes still list its true
  historical parent's hash (`git cat-file -p` on the boundary commit
  shows it), but that parent object is genuinely absent locally by
  design (`git cat-file -t <oid>` fails identically) — real `git blame`
  succeeds because it consults `.git/shallow` and treats the boundary
  commit as having no parents, while `gix-blame` 0.16.0 does not and
  tries to fetch the missing object anyway. This is a real, reproducible
  `gix` maturity gap, not a `lux-git` bug — no shell-out workaround was
  applied (ADR 0010 requires its own superseding ADR before that path,
  and this doesn't warrant one): shallow clones are a narrow case (CI
  checkouts, not the ordinary "open a project in an editor" scenario a
  full clone represents), so this is documented as a known limitation in
  `docs/STATUS.md` rather than worked around.

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git` —
  74 tests, 0 failures, 0 ignored (up from 60 before this loop: 4 new
  `status` tests, 4 new `diff_view::inline_diff` tests moved with the
  module split, 5 new `staging` per-hunk tests, 1 new `diff_panel`
  gitignore-vs-tracked regression test, 1 new `stash` staged-file
  regression test). Full workspace gate clean: `cargo test --workspace`
  (0 failures across every crate), `cargo clippy --workspace -- -D
  warnings`, `cargo deny check` (advisories/bans/licenses/sources all
  ok — the pre-existing `windows_*`/`bitflags` duplicate-version warnings
  are unrelated desktop-platform transitive dependencies, not introduced
  here), `./scripts/check-limits.sh` (`diff_panel.rs` 383 lines,
  `diff_view.rs` 328 lines, `stash.rs` 469 lines, `staging.rs` 391 lines —
  every `lux-git` file under the 600-line cap), and
  `./scripts/check-core-isolation.sh`. Separately, the manual
  large-repository pass against a real shallow clone of `rust-lang/rust`
  completed end-to-end (status, changed-files walk, gutter diff and
  markers, side-by-side/inline diff, per-hunk staging, a real commit,
  history walk, branch create/switch materializing the full ~62,000-file
  tree, stash save/pop, and a real induced three-way merge with conflict
  detection and resolution) — every operation succeeded except the
  documented shallow-clone blame limitation above, and switch/stash/merge
  each completed in single-digit seconds against the full tree (real,
  useful numbers for whoever eventually wires this into `lux-ui` and
  needs to decide what belongs off the UI thread, per golden rule 6 —
  not a new rule this loop is asserting, the existing one already
  requires it).

  **What's still open, and why it's staying open rather than being
  silently declared done:**

  - The "Accept Current / Incoming / Both" editor commands themselves
    (the actual UI actions named in the master prompt's Phase 6 bullet)
    don't exist and can't yet, because `lux-ui` doesn't consume `lux-git`
    at all — building that integration is Phase 6 UI-wiring work, a
    different and much larger scope than this loop's "finish `lux-git`'s
    pending backend pieces," and every capability in this entire
    `docs/STATUS.md` section already carries the same "not wired to
    `lux-ui`" caveat for the same underlying reason.
  - Rename and delete/modify merge conflicts (real `gix-merge` outputs
    this crate doesn't yet turn into a resolvable `MergeConflict`) —
    unchanged from the prior loop's documented scope note.
  - The `gix-blame`/shallow-clone limitation above.
  - No row in `docs/STATUS.md`'s Phase 6 table moved to `implemented` —
    same convention as every row in this document: real, tested backend
    capability, nothing wired into the running `lux` binary.
