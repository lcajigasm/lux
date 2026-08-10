# Receipt: Language-aware auto-indent (deliverable 5)

- Phase: 03
- Crate(s): lux-syntax
- Goal: The "Language-aware auto-indent" deliverable of Phase 3 ("Syntax,
  navigation, and palette") of `LUX_PROMPT_MAESTRO.md` is complete and its gate
  passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-syntax indent_rules::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-08
- Closed: 2026-08-08
- Notes:

  **Design.** `crates/lux-syntax/src/indent_rules.rs`, a new module,
  following the same pattern as `brackets.rs`/`folding.rs`: it operates
  directly on `tree.root_node()`, with no dependency on `lux-core` or
  on any UI. `indent_level(&Document, source, row) -> usize` returns
  the indentation level (not spaces/tabs — that conversion remains the
  responsibility of `lux-core::buffer::Indent`, which already exists and was
  not touched) by counting how many of the language's "body nodes" (a
  `block`, a `compound_statement`, an `object`, ...) enclose the target
  row.

  **Why it isn't "count all ancestors".** The first idea — counting
  distinct start rows among the ancestors of the point — works for Rust/C
  (the header and its `{` share a row, so they deduplicate on their own)
  but gives incorrect results in Python (`if_statement` starts on the
  `if`'s row, its child `block` starts one row later: they're the same
  indentation level but count as two) and in Swift (`function_body`
  and its inner `statements` also don't share a row). The solution that
  does generalize: each grammar has a fixed, small set of *kinds*
  that represent "an indented body" — established by inspecting the
  actual tree of each vendored grammar (the same way
  `queries/<lang>/highlights.scm` establishes the capture names for each
  one), not guessed. Container/header nodes (`if_expression`,
  `else_clause`, `elif_clause`...) are never in the list, so they never
  add a level on their own — which also makes an `else`/`elif` in
  Python dedent correctly with no keyword-specific special case.

  **Body closing (`}`, `fi`, `done`).** A line whose first non-blank token
  is literally the last token of the body enclosing it dedents to the
  level of the line that opened that body, not to the level inside it.
  Detected generically (with no hardcoded list of closing characters):
  the leaf node found is the child of a node whose *kind* is in the
  language's "body" list and ends exactly where that parent ends
  (`parent.end_byte() == leaf.end_byte()`). This covers both braces
  (`}`/`)`/`]`) and closing keywords (`fi` in Bash, `done` in Bash) with
  the same rule.

  **YAML/Markdown fix.** Both grammars wrap the file's top-level content
  in the same *kind* they use for real nesting (`block_mapping`/
  `block_sequence` in YAML, `list` in Markdown) — even though that top
  level has no opening delimiter and its direct children sit at column
  0, just like the file itself. This was discovered while writing the
  YAML test: a first-level key came out with level 1 instead of 0.
  JSON/HTML don't have this problem (their outermost node has a real
  `{`/`<div>` and its content genuinely is indented one level).
  Fixed by subtracting 1 only for those two languages (`root_offset`),
  documented in the code itself with the reason, not as a silent
  `#[allow]`.

  **Empirical verification before coding.** Instead of guessing *kind*
  names per language, a probe test was temporarily added
  (`probe_tmp`, removed before finishing) that dumped the actual tree
  of each vendored grammar for representative samples (Rust, Python,
  C/C++, Java, JS/TS, Go, Kotlin, Swift, HTML, CSS, JSON, YAML, Markdown,
  Bash, TOML), including class+method cases and bash with
  if/elif/else/fi + for/do/done. The 18 `block_kinds` sets in the
  final code come from that real inspection, not from each grammar's
  documentation.

  **Known limitation, documented (not hidden).** The tree-sitter-bash
  grammar does not wrap an `if`/`then`'s main body in its own node —
  `if_statement` does double duty as header and body, and its
  `elif`/`else` clauses are its siblings, not nested inside another
  countable *kind*. This means the body of `if`/`for` and the closing
  `fi`/`done` are computed correctly (verified by tests), but
  dedenting the `elif`/`else` line itself to the `if`'s level isn't
  specially derived — it's left documented with a `ponytail:` comment
  in the code instead of building a branch-keyword mechanism just for
  this one grammar. None of the other 17 languages has this
  limitation.

  **Real verification.** `cargo test -p lux-syntax indent_rules::`: 18/18
  passed, 0 ignored. The tests are non-trivial: they cover real deep
  nesting in Rust/C with header and brace on the same row, a Python
  block with no braces at all, Python `else` dedenting with no special
  case, a `{` inside a string literal (never counted as a real
  bracket), blank lines inside a body, a closing brace dedenting to the
  line that opened it, full Bash `if/then/fi` and `for/do/done`,
  nested JSON/YAML, and the YAML/Markdown fix described above. Full
  repo gate green: `cargo run -p xtask` (fmt, clippy `-D warnings` across
  the whole workspace, `cargo test --workspace`: 235 tests in
  `lux-core` plus the rest of the workspace with no regressions,
  `cargo deny check`, file limits, `lux-core` isolation). The four
  previous deliverables of this phase (`highlight::`, `theme::`,
  `folding::`, `brackets::`) were re-verified intact in the same pass.
  `crates/lux-syntax/src/indent_rules.rs` is 276 lines, well under the
  600-line limit; no field was added to any struct and no new
  dependency was added — only a `pub(crate) fn language()` method
  on `Document`, analogous to the `tree()` that already existed for
  `brackets`/`folding`.

  **What's out of scope for this deliverable** (rest of Phase 3, untouched):
  Goto Anything, command palette, search/replace bar,
  tabs/status bar, minimap — and, within auto-indent's own scope
  itself, the wiring to an actual editor command (Enter key,
  reindent selection) in `lux-ui`, which doesn't exist yet in this phase
  (there's no editing UI consuming this yet) and is therefore
  later work, not this library's. Verified only on macOS, the same
  limitation documented elsewhere in `docs/STATUS.md`.
