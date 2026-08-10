# Receipt: Minimap (deliverable 10, last of Phase 3)

- Phase: 03
- Crate(s): lux-ui
- Goal: The "Minimap" deliverable of Phase 3 ("Syntax, navigation and
  palette") from `LUX_PROMPT_MAESTRO.md` is complete and its gate passes
  with no failures and no ignored tests.
- Verify: `cargo test -p lux-ui minimap::`
- Iterations: 1 / 6
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt devotes a single line to this deliverable:
  "Optional minimap." — with no acceptance criteria of its own (it does not
  appear in the Phase 3 "Acceptance criteria" list) and no behavioral
  detail at all. With that as context, the design leaned on what already
  exists in the crate itself instead of inventing from scratch: like the
  five previous deliverables of this phase, there is no real interactive UI
  in `lux-ui` yet, so this is the geometry/state layer that a minimap UI
  would consume, explicitly following the same pattern `EditorView` already
  documents about itself ("produces the geometry a render pass would draw,
  not pixels"). `docs/STATUS.md` keeps the "Minimap" row at `not-started`
  (rule 1) with the prose detailing what was built.

  **The actual design decision behind this deliverable: never shape
  glyphs.** `EditorView` (Phase 2, deliverable 4) already documents why it
  only shapes visible lines: shaping an entire 50k-line document every
  frame would be exactly the cost virtualization exists to avoid. A
  minimap that rendered real text from all 50,000 lines every frame would
  silently reintroduce that same cost, just under another name. That's why
  `minimap.rs` reduces each line to a "density bar": indentation offset +
  content length, both counted in characters from `LineIndex` (the same
  byte-range line splitter `EditorView` already uses, re-exported as
  `pub(crate)` from `editor_view::line_index` — zero new dependency on
  `lux-render`/shaping). It's the same real technique used by production
  minimaps (a blurred approximation of the text, not the text).

  **Downsampling with no empty trailing rows — not the most obvious
  approach.** When the document has more lines than available pixel rows,
  several lines collapse into one row. The first design
  (`lines_per_row = ceil(total/available_rows)`, fixing `available_rows`
  exact rows) leaves empty trailing rows when the division isn't exact —
  not a correctness bug (no line is lost and there's no panic), but a real
  visual defect (the minimap would not reach the bottom). Distributing the
  remainder across buckets was considered (the "textbook" approach for
  spreading N items across K buckets with no gaps) but was dropped as more
  code for a deliverable with no behavioral specification at all. The
  chosen solution: instead of fixing the number of rows to
  `available_rows`, it computes how many rows are actually needed
  (`row_count = ceil(total / lines_per_row) <= available_rows`) and only
  generates those — the minimap can use fewer rows than the available
  maximum, but never leaves a row half-filled or empty. Explicitly verified
  in `build_frame_never_pads_with_empty_trailing_rows`.

  **The row represents the "densest" line of the group, not the first nor
  an average.** Picking the first line of each group would be simpler but
  could hide a genuinely long line if it falls after a blank line within
  the same bucket. Averaging doesn't make sense either for an integer
  length metric. The group is walked and the line with the highest
  `content_columns` is kept — same cost (a `for` over the group, already
  needed to compute any variant), but correct on the edge case that
  actually matters (a long line must not disappear from the minimap due to
  bad bucketing luck).

  **`indent`/`Indent` was removed from the signature after a first
  attempt.** The first version passed `Buffer::indent()` into `densest_row`
  "for symmetry" with `editor_view::indent_guide_x`, but `indent_columns`
  counts the line's own actual whitespace, it doesn't need the buffer's
  detected style/width — the parameter wasn't used at all. It was removed
  entirely (parameter, import, and the call to `buffer.indent()`) instead
  of leaving it with a `let _ = indent;` silencing the warning; no
  abstraction without real use, not even "just in case."

  **`line_for_row`/`row_for_line`: the click-to-navigate primitive.**
  Although there is no mouse event handling yet, it is exposed as a pure
  function (`total_lines, available_rows, row -> line`) — the same pattern
  already used in `search_bar`/`tabs` for navigation primitives with no
  real UI yet: the logic is ready and tested, only a real click event needs
  to be wired in once that layer exists.

  **A real failure during development, not of the design but of line
  counting.** The test `build_frame_never_pads_with_empty_trailing_rows`
  expected 2 rows for the buffer `b"1\n2\n"`, but that buffer has 3 lines
  (the trailing newline adds an extra empty line, the same `ropey`/
  `LineIndex` convention that module already documents, and the same kind
  of counting mistake that already showed up in the search bar receipt of
  an earlier loop). Fixed after re-reading the convention documented in
  `line_index.rs` instead of assuming.

  **Actual verification.** `cargo test -p lux-ui minimap::`: 12/12 passed, 0
  ignored (enabled by default, `toggle`, `None` when disabled, 1:1 mapping
  when the document fits, downsampling that picks the densest line,
  absence of empty trailing rows both when downsampling and when there are
  more available rows than lines, viewport indicator at 1:1 and across
  downsampling, empty indicator for an empty visible range, click-to-line
  with the normal case and with an out-of-range row saturated to the last
  line, empty document as a single empty row). One real failure during
  development (above), fixed and reverified. Full repo gate green:
  `cargo run -p xtask` (fmt — one adjustment after writing the file,
  applied with `cargo fmt`; clippy `-D warnings` across the whole workspace
  with no findings; `cargo test --workspace`: 235 tests in `lux-core` +
  the rest of the workspace with no regressions, 95 total in `lux-ui` (83
  previous + 12 new); `cargo deny check`; file limits — `minimap.rs` at
  312 lines, well under the 600 limit; `lux-core` isolation). No new struct
  exceeds 3 fields. No new dependency was added — everything built on
  `lux_core::buffer::Buffer` and the `LineIndex` already existing in
  `lux-ui`. Verified on macOS only, same documented limitation as the rest
  of `docs/STATUS.md`.

  **Closing out Phase 3.** This is deliverable number 10 and the last on
  the Phase 3 list in `LUX_PROMPT_MAESTRO.md`. All ten now have their
  core/state implemented and tested (`lux-syntax` for the first five;
  `lux-ui::goto_anything`, `command_palette`, `search_bar`,
  `tabs`/`status_bar`, and `minimap` for the last five) — but
  `docs/STATUS.md` still marks as `not-started` any row whose real
  interaction part (UI, mouse/keyboard events, menu wiring, pixel
  rendering) doesn't exist yet, consistent across the whole phase. Formally
  closing Phase 3 (verifying the header's "Acceptance criteria" —
  reparse/latency budgets, the `v0.2.0` cut — and writing its release
  notes) is not part of this specific deliverable and has not been
  touched.

  **What's out of scope for this deliverable** (not touched): the real
  minimap UI (rendering, click/drag handling — no interaction UI exists in
  `lux-ui` yet, the same state left by the previous five receipts of this
  phase). The pending work from previous deliverables that was already in
  the tree when this loop started was left untouched, neither modified nor
  committed (auto-indent in `lux-syntax`, and Goto Anything / command
  palette / search bar / tabs + status bar from the four previous loops,
  each with its own receipt, still uncommitted).
