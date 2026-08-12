# Receipt: Indicadores de cambio en el gutter + navegación de hunks (Phase 6)

- Phase: 06
- Crate(s): lux-git
- Goal: The "Indicadores de cambio en el gutter + navegación de hunks"
  deliverable of Phase 6 ("Git integrado con `gix`") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-git gutter::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-11
- Closed: 2026-08-11
- Notes:

  **Scope.** `lux-git` didn't exist yet beyond an empty crate skeleton
  (`Cargo.toml` with no dependencies, a one-byte `lib.rs`). This loop adds
  the `gutter` module only — diff computation, per-line gutter markers,
  and hunk navigation — nothing else from Phase 6's much longer
  deliverable list (branch/status bar, staged-changes tree, diff panel,
  staging, commit, history, blame, branches, stash, merge conflicts) and
  no wiring into `lux-ui`'s actual gutter rendering (`crates/lux-ui/src/
  editor_view/mod.rs` currently only computes line-number width; it isn't
  touched here). `docs/STATUS.md`'s "Gutter diff indicators" row stays
  `not-started`, same convention used throughout Phases 4-5: a real,
  tested library capability with nothing wired into the running `lux`
  binary doesn't move the row — see the scope note added to `docs/
  STATUS.md`'s Phase 6 section.

  **Dependency: `gix` 0.86.0, exactly as ADR 0010 pins it**, with
  `default-features = false` and only the features actually needed —
  `blob-diff` (pulls in `gix-diff`'s blob/`imara-diff` machinery, which
  transitively re-exports as `gix::diff::blob`), `revision`, and `sha1`
  (`gix-hash` refuses to compile without picking a hash implementation
  explicitly once default features are off). No shell-out to `git`
  anywhere, including in tests — the integration tests build their fixture
  repositories with `gix`'s own object-writing API (`write_blob`,
  `write_object`, `commit_as`), not the `git` CLI.

  **Diffing reuses `gix`'s own engine instead of hand-rolling one.**
  `gix::diff::blob::{InternedInput, diff_with_slider_heuristics,
  Algorithm::Histogram}` is the same histogram-diff-plus-slider-heuristics
  path `gix` uses internally for its own diffs — not `UnifiedDiff`/
  `ConsumeHunk` (built for rendering unified-diff *text*, with configurable
  context lines) but the lower-level `Diff::hunks()` iterator, which
  yields raw contiguous change ranges with no context padding — the right
  shape for gutter markers, where every changed line needs its own
  decoration, not a diff to print.

  **Hunk model.** `Hunk { before_start, before_len, after_start,
  after_len }`, 1-based, mirroring the convention `gix_diff::blob::
  unified_diff::HunkHeader` itself uses (`token_index + 1`, uniformly,
  regardless of hunk length) rather than classic unified-diff's
  cosmetic off-by-one for zero-length sides — there's no textual header
  being rendered here, so there's no reason to replicate that quirk.
  `Hunk::kind()` classifies `Added` (`before_len == 0`), `Removed`
  (`after_len == 0`), or `Modified` (otherwise). `Hunk::anchor_line()` is
  the line a hunk attaches to for gutter placement and navigation — for a
  pure deletion (which owns no line in the working copy) that's the line
  immediately after the deletion point, the same place most editors render
  a "removed" indicator.

  **`hunks_against_head`** reads a path's blob at `HEAD` via
  `repo.head_tree_id_or_empty()` (an unborn `HEAD` diffs against nothing,
  rather than erroring out on a brand-new repo with no commits) and a
  path missing from that tree diffs against an empty string, so a new
  file comes back as a single `Added` hunk covering the whole thing — both
  cases are covered by `against_real_repo::unborn_head_diffs_against_empty`
  and `against_real_repo::new_file_not_yet_at_head_is_all_added`.

  **What tripped the first pass, fixed within the same iteration (not
  worth a second loop):** two test-assertion bugs, not implementation
  bugs. `deletion_marker_anchors_after_the_deletion_point` asserted the
  marker on the wrong line (off by one — hadn't re-derived
  `anchor_line()`'s 1-based math by hand before writing the assertion).
  `against_real_repo::diffs_working_copy_against_head`'s fixture committed
  a file at path `"src/lib.rs"` as one flat tree entry with a `/` in its
  filename instead of a real nested `src/` tree — `gix`'s own
  `lookup_entry_by_path` (correctly) couldn't find a component-wise
  `src/lib.rs`, silently fell back to "file absent at HEAD," and the test
  then asserted a wrong line number against what became a whole-file
  `Added` hunk. Fixed by flattening the fixture path to `"lib.rs"` — this
  loop doesn't test nested-tree construction, so building one properly
  (multiple tree objects, one per directory level) would have been scope
  creep for a test fixture. No durable correction worth promoting to
  `CLAUDE.md`; both were one-off test-authoring mistakes, not a systemic
  gix or design pitfall.

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git
  gutter::` — 13 tests, 0 failures, 0 ignored (10 pure-function tests
  covering diffing, marker expansion, and navigation including wraparound
  and the empty/single-hunk edge cases; 3 integration tests against real
  temporary `gix` repositories). Also clean: `cargo clippy -p lux-git
  --all-targets -- -D warnings`, `cargo fmt --check -p lux-git`,
  `./scripts/check-limits.sh` (416 lines, well under the 600-line cap),
  and `cargo build --workspace` (confirms the new `gix` dependency graph
  doesn't break any other crate's resolution).
