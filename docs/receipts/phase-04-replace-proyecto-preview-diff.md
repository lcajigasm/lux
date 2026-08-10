# Receipt: Replace in project, with diff preview (deliverable 5 of Phase 4)

- Phase: 04
- Crate(s): lux-search
- Goal: The "Replace in project with diff preview" deliverable of Phase 4
  ("Projects, files and global search") from `LUX_PROMPT_MAESTRO.md` is
  complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-search replace_preview::`
- Iterations: 2 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text: "Replace in project
  with diff preview and confirmation by file or by hunk. Never a blind
  mass replace." Only this bullet was in scope — not the incremental
  index, sessions/recovery, or huge/binary file handling (deliverables
  6-8). `docs/STATUS.md`'s "Replace in project with diff preview" row
  stays `not-started` (rule 1): no diff/confirmation UI exists in
  `lux-ui`, nothing wired into the `lux` binary — this is the engine such
  a UI would call into.

  **Deliberately built as a pure function over `find_in_files`'s output,
  not a second search implementation.** `build_preview(matches, pattern,
  replacement, case_insensitive)` takes an already-collected
  `&[SearchMatch]` (typically `SearchStream::matches()` after a
  `find_in_files` search from the previous loop) instead of re-walking the
  project itself. This keeps the module's only I/O in one place (`apply`,
  the actual write step) and reuses the traversal/`.gitignore`/glob
  machinery deliverable 4 already built and tested, rather than
  duplicating it. The replacement text itself is computed with the plain
  `regex` crate (already a dependency of `lux-core`, same 1.13.1 — `cargo
  tree -i regex` confirms one resolved version, no duplicate-version
  warning), the same tool `lux_core::find_replace` already uses for
  in-buffer replace, so `$1`/`${name}` capture-group syntax behaves
  identically at both layers instead of inventing a second replacement
  syntax for the project-wide case.

  **"Never a blind mass replace" is the actual design, not a docstring
  promise.** Every `ReplacementHunk` computed by `build_preview` starts
  `accepted: false`. There's no "replace everything" default path —
  `apply()` only ever touches hunks the caller explicitly opted in via
  `accept`/`accept_all_in_file`; a preview with nothing accepted applies
  to zero files (`apply_with_no_accepted_hunks_touches_nothing`).
  "Confirmation by file or by hunk," the deliverable's other named
  requirement, is `accept`/`reject` (one hunk) versus
  `accept_all_in_file`/`reject_all_in_file` (every hunk in one file) —
  both operating on the same underlying state, so a UI can offer either
  granularity without two separate code paths.

  **The second, sharper half of "never blind": a conflict check right
  before every write, not just an opt-in gate.** A hunk's `before` field
  is the line's content *as read at preview time*. `apply` re-reads the
  file and compares that exact line against the file's current content
  immediately before overwriting it; if it no longer matches (something
  else — the user, another tool, an external process — changed that line
  since the preview was built), the hunk comes back as `conflicted`
  instead of silently clobbering whatever's there now. Verified directly
  (`apply_detects_a_conflict_when_the_file_changed_since_the_preview`):
  the file changes between `build_preview` and `apply`, and the test
  confirms both that the hunk is reported as conflicted *and* that the
  file was never rewritten as a result.

  **Diff preview, scoped to what's actually needed and testable at this
  headless layer.** No unified-diff-string dependency was added
  (`similar` or equivalent) — the deliverable's "diff preview" is
  satisfied by exposing `before`/`after` per hunk as plain data, the same
  "geometry a render pass would draw, not pixels" pattern
  `lux_ui::editor_view`/`file_tree`/`search_bar` all already establish for
  their own not-yet-built UIs. A future diff panel renders `before` next
  to `after` directly; there's no intermediate diff-format string to
  generate or parse for that.

  **Writes are grouped one-per-file even with several accepted hunks in
  the same file**, and use a hand-written `split_keepends` (no stdlib
  equivalent — `str::lines()` strips terminators and doesn't distinguish
  `\r\n`) so line endings round-trip exactly for every line *not*
  touched by an accepted hunk, and `\r\n` files stay `\r\n` after a
  replace. Verified directly
  (`apply_groups_multiple_accepted_hunks_in_the_same_file_into_one_write`,
  `preserves_crlf_line_endings`).

  **A real, if small, first-run failure — a bug in the test's own math,
  not the implementation.** `build_preview_expands_capture_groups` first
  asserted `"name: alice, id: 7\n"` for pattern `(\w+): (\w+)` with
  replacement `$2=$1` — but swapping key/value in "name: alice" correctly
  produces `"alice=name"`, not the original text back. The regex engine
  was right; the test's expected value was backwards. Fixed by correcting
  the assertion to `"alice=name, 7=id\n"` with a comment explaining the
  swap, not by touching `build_preview` itself (12/13 tests passed on the
  very first run, this one included).

  **Actual verification.** `cargo test -p lux-search replace_preview::`:
  13/13 passed, 0 ignored, on the second run (the capture-group test fix
  above was the only iteration needed). Coverage: one hunk per match with
  correct before/after; capture-group expansion; case-insensitive
  replacement; an invalid pattern surfacing as `SearchError::InvalidPattern`
  before any hunks are built; accept/reject toggling what
  `accepted_hunks()`/`apply()` include; accept/reject on an unknown
  `HunkId` returning `false`; `accept_all_in_file` scoped correctly to one
  file among several; a real on-disk apply that writes only accepted
  lines and leaves rejected ones untouched; multiple accepted hunks in one
  file collapsing to a single write; the conflict-detection case above; a
  missing file reported as `FileApplyOutcome::Failed` rather than a panic
  or a silent no-op; an all-rejected preview touching nothing; CRLF
  preservation. Full repo gate also run and green: `cargo run -p xtask` —
  `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --
  -D warnings` (zero warnings); `cargo test --workspace` (all crates, no
  regressions — `lux-search` now at 32 tests total: 8 `fuzzy::` + 11
  `find_in_files::` from the prior loop + 13 new `replace_preview::`,
  every other crate's count unchanged); `cargo deny check`
  (advisories/bans/licenses/sources all `ok`, no new `deny.toml` entries
  needed — `regex` is already MIT/Apache-2.0-allowed and already in the
  tree at the same version via `lux-core`); `scripts/check-limits.sh`
  (`replace_preview.rs` at 473 lines, under the 600 cap; its largest
  struct, `ReplacementHunk`, has 6 fields, well under 25);
  `scripts/check-core-isolation.sh` doesn't apply (only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only.

  **What's out of scope and untouched:** the incremental index,
  sessions/recovery, and huge/binary file handling (Phase 4 deliverables
  6-8); a real diff/confirmation panel in `lux-ui` (no interactive UI
  exists there yet — this deliverable is the engine such a panel would
  call into); wiring `build_preview`/`apply` into the `lux` binary or into
  `find_in_files` itself (they stay two composable, independently testable
  pieces on purpose); Windows/Linux verification (no non-macOS dev
  environment available); atomic (temp-file-plus-rename) writes in
  `apply` — `lux_core::buffer::Buffer::save` uses that pattern for the
  single-buffer case, but a straightforward `fs::write` was judged
  sufficient here given `apply`'s own conflict check already guards
  against the more likely failure mode (concurrent external edits); worth
  revisiting if a crash-during-write partial-file case ever turns out to
  matter in practice.
