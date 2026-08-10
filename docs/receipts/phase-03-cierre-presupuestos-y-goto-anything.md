# Receipt: Phase 3 closure — performance budgets and Goto Anything gaps

- Phase: 03
- Crate(s): lux-syntax, lux-search (indirect), lux-ui, benches
- Goal: The blockers found when trying to close Phase 3 with
  `/close-phase 3` — the two phase performance budgets that had never been
  measured, the 16 ms highlighting criterion that had never been verified,
  and the pending Goto Anything sub-suffixes — are resolved: measured with
  real data where measurable, fixed where there was a genuine architecture
  problem, and either implemented or formally deferred via ADR where
  appropriate.
- Verify: `cargo bench -p lux-benches --bench phase3_budgets` (budgets);
  `cargo test -p lux-syntax` (new `highlight_range` + `highlight`
  intact); `cargo test -p lux-ui goto_anything::` (history +
  preview); `cargo run -p xtask` (full repo gate)
- Iterations: 1 / — (this wasn't a loop with a single deliverable; see notes)
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Context.** Running `/close-phase 3`, step 2 (checking receipts)
  technically passed — all 9 receipts for the phase say
  `Result: passed` — but cross-checking against the phase's own
  acceptance criteria and against `docs/STATUS.md` revealed real gaps:
  two of the four acceptance criteria had never been measured, a
  third was literally impossible to verify given the code as it stood,
  and deliverable 6 (Goto Anything) had pending sub-suffixes that were
  unresolved. Work stopped there instead of closing the phase by
  simulating a state that `docs/STATUS.md` contradicted. This loop
  resolves those blockers.

  **1. Performance budgets measured for the first time.**
  `benches/benches/phase3_budgets.rs` (new, in the `lux-benches` crate,
  same pattern as `phase1_budgets.rs`): generates 10k/20k lines of real,
  parseable Rust, builds a correct `tree_sitter::InputEdit` by hand
  (`Document::edit` had never been exercised in any `lux-syntax` test
  until now — not even in `golden_tokens.rs`) and measures
  `Document::edit` in isolation for the reparse budget. For
  Goto Anything, it generates 100,000 synthetic paths and measures
  `lux_search::fuzzy::rank_paths` directly.

  **Real results** (see `docs/PERFORMANCE.md` for the full table with
  date and context): reparse after a keystroke on 10k lines, **1.05 ms**
  against a 2 ms budget — passes. Goto Anything over 100k files,
  **1.34 ms** against a 150 ms budget — passes with more than 100×
  margin (with the documented caveat that "first results < 30 ms"
  is not a separately measurable phase in a synchronous implementation;
  see below).

  **2. The 16 ms criterion revealed a real architecture problem,
  not just a missing measurement.** The first measurement of reparse+highlight
  after a keystroke on a 20k-line file came in at **~24.6 ms** — well
  over budget. This was investigated rather than accepted as "fine, we'll
  fix it later": `Document::highlight` walks the **entire** tree on
  every call (`cursor.captures(&query, tree.root_node(), ...)`, with no
  range bound), so every keystroke would re-highlight the whole
  document, not just what's visible — exactly what `EditorView`'s
  virtualization (Phase 2, deliverable 4) exists to avoid in glyph
  shaping, but which highlighting had never inherited.

  **A real fix, not a benchmark adjustment.** Added
  `Document::highlight_range(source, byte_range)` in
  `crates/lux-syntax/src/lib.rs`, using
  `tree_sitter::QueryCursor::set_byte_range` (this genuinely bounds what
  the cursor walks, it is not a post-hoc filter over already-computed
  results). `highlight()` now delegates to
  `highlight_range(source, 0..source.len())` — verified byte-identical
  against the 18 tests in `golden_tokens.rs`, which remain intact with
  not a single fixture file modified. A second finding turned up while
  measuring the version bounded to 50 lines (~11.6 ms — better, but
  surprisingly high for just 50 lines): `Query::new` — compiling the
  full highlight pattern from the `.scm`, hundreds of lines — was being
  recompiled on **every** call to `highlight`/`highlight_range`,
  and turned out to be the dominant cost, not the tree walk. It was
  cached as a `Document` field (`highlight_query: Query`, built once in
  `Document::new`). Combined effect of both fixes: full
  document, from ~26 ms to ~16.8 ms (with query caching but no
  range bound — still hovering at/over budget, kept deliberately in the
  benchmark as proof of why bounding the range is necessary, not an
  alternative to it); 50-line visible range, from ~11.6 ms
  to **~2.3 ms** — the real number that must meet the budget, with
  ample margin.

  **3. `@symbol` and `#text`: deferred with an ADR, not simply
  missing.** Before implementing them, what each would actually
  require was checked: `@symbol` needs a real symbol index
  (LSP `workspace/symbol`, Phase 7, or per-grammar outline queries
  in `lux-syntax`, neither of which exist and would be genuinely new
  scope); `#text` needs full-text search across the whole project, which
  is literally deliverable 4 of Phase 4 ("Find in Files with the
  embedded ripgrep engine"). Building them now would mean either
  throwaway infrastructure that Phase 4/7 would replace, or pulling
  those phases forward on our own initiative — rule 7 ("Scope is
  closed... do not extend the roadmap on your own initiative") explicitly
  forbids that. The decision was documented in
  `docs/adr/0014-goto-anything-symbol-text-suffixes-deferred.md`, with
  what unblocks each suffix and why, so whoever picks this up once
  Phase 4/7 exist doesn't have to re-derive the reasoning.

  **4. Recent-files history and preview, actually implemented —
  they don't depend on any future phase.** `RecentFiles` in
  `crates/lux-ui/src/goto_anything.rs`: the same logical clock as
  `command_palette::CommandRegistry` (no real timestamp, just relative
  order, deterministic and testable without `sleep`). `search_with_recency`:
  empty query → list by recency (never-opened items last, in their
  original order); non-empty query → fuzzy score first, recency
  only breaks ties — a clearly better fuzzy match never loses to a
  worse one just because it was opened longer ago (explicitly verified
  in a test, after a first attempt with test data where the intuition
  about `nucleo`'s scoring turned out wrong — see below).
  `preview(buffer, line, radius_lines)`: a text snippet around the
  target line, from a `Buffer` already loaded by the caller —
  deliberately does **not** open the file itself, because that would be
  I/O on what would eventually be the UI thread (golden rule
  #6); reading the file remains the caller's responsibility,
  same as every other module in this crate.

  **A real failure during development.** The first test for "recency
  must not beat a clearly better fuzzy match" used
  `"b/mainframe_unrelated_padding.rs"` as the worse candidate — it
  failed: `nucleo`'s real scoring rated it higher than expected (the
  path-boundary bonus after `/` plus the contiguous match of "main" at
  the start of "mainframe" competes more closely than intuited against
  "a/main.rs"). Rather than adjusting the threshold or weakening the
  assertion, the candidate was changed to one where "main" only matches
  with wide gaps (`"b/manual_index.rs"`), an unambiguous match-quality
  difference, and the test passed for the right reason.

  **Real verification.** `cargo bench -p lux-benches --bench
  phase3_budgets`: the four numbers above, reproduced after each
  code change (not just once). `cargo test -p lux-syntax`: 44 unit
  tests + 18 `golden_tokens` = 62, all green, including 3 new
  `highlight_range` tests (delegates the same as `highlight` over the
  whole document, excludes tokens outside the range, empty before the
  first parse). `cargo test -p lux-ui goto_anything::`: 17/17 (10 prior +
  7 new for `RecentFiles`/`search_with_recency`/`preview`). Full
  repo gate green: `cargo run -p xtask` (fmt, clippy
  `-D warnings` across the whole workspace with no findings after
  formatting adjustments; `cargo test --workspace`: 235 in `lux-core` +
  the rest of the workspace with no regressions, 102 in `lux-ui` (95
  prior + 7 net new, with 10 replaced by 17), 44+18 in `lux-syntax`;
  `cargo deny check` — no new duplicates; file limits —
  `crates/lux-syntax/src/lib.rs` 376 lines, `crates/lux-ui/src/
  goto_anything.rs` 356 lines, `benches/benches/phase3_budgets.rs` 246
  lines, all well below the 600 limit; `lux-core` isolation).
  `Document` goes from 3 to 4 fields (adds `highlight_query`), well
  below the 25 limit.

  **What remains unresolved, on purpose.** The real structural gap
  left after this loop is not about performance or Goto Anything
  scope: **no Phase 3 deliverable is wired to a real UI yet** — not
  a single highlighted token paints on screen, there is no `Cmd+P` or
  `Cmd+Shift+P` dialog, no tab renders.
  It's the same state every prior receipt in this phase documented,
  and fixing it isn't a phase-closure cleanup — it's building the
  real interaction wiring for five distinct subsystems, each with its
  own scope and its own design decisions. It is left out of
  this loop deliberately, and is reported as the actual pending decision
  before tagging `v0.2.0` (see this session's closing message).
