# Receipt: Panel de preferencias, o edición de fichero con autocompletado del esquema (deliverable 6 of Phase 5)

- Phase: 05
- Crate(s): lux-config
- Goal: The "Panel de preferencias, o edición de fichero con
  autocompletado del esquema (elige uno; documenta la elección)"
  deliverable of Phase 5 ("Configuración, keymaps y personalización") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests. (This loop's `/goal` was open-ended — "aborda el
  entregable 6" — with no explicit verify command or iteration cap; this
  receipt records the verify command and scope this loop actually settled
  on.)
- Verify: `cargo test -p lux-config` (no single module filter covers both
  new modules in one `cargo test` invocation — `schema::` and
  `completion::` together — since `cargo test` accepts exactly one
  `TESTNAME` filter; the whole-crate run is what was actually re-run
  repeatedly to confirm green)
- Iterations: 1 (no cap was set for this loop; two bugs were found and
  fixed by tracing the state machine by hand before the first test run,
  not by a failing run-fix-rerun cycle — see Notes)
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Why this loop exists.** `/close-phase 5` was attempted first and
  stopped early per its own step 2: deliverable 6 had no receipt at all —
  nothing had been built for it yet, unlike deliverables 1-5 which all had
  a `Result: passed` receipt (deliverable 1's receipt also flagged a
  second, separate gap — schema publication and line/column validation —
  which this loop does not close; see the last paragraph of these notes).
  This loop is that missing deliverable 6.

  **The choice, as the deliverable's own text requires ("elige uno;
  documenta la elección"): file editing with schema-aware completion, not
  a preferences UI panel.** Documented at length in `completion.rs`'s own
  module doc comment, summarized here: every other Phase 5 deliverable in
  this crate — `layering`, `keymap`, `editorconfig`, `gitattributes`,
  `import`, `font_settings` — is headless, UI-free logic with zero
  graphics dependency, matching golden rule #2's spirit even though the
  rule's literal CI gate (`scripts/check-core-isolation.sh`) only checks
  `lux-core`. A preferences *panel* would need real `lux-ui`/`lux` widget
  and rendering infrastructure that doesn't exist yet for this specific
  purpose, and building it inside `lux-config` would be both a scope
  mismatch (a domain crate reaching for a graphics dependency it's never
  needed) and inconsistent with the pattern every other loop this phase
  already established. File-editing completion, by contrast, is exactly
  the shape of thing this crate already does: pure text-and-schema logic a
  future editor surface can call into — "produce the data, wiring it into
  the `lux` binary is a future integration's job," the same sentence every
  earlier Phase 5 receipt already used.

  **`schema.rs`: the first place this crate's settings vocabulary is
  written down as one list, not new vocabulary.** All ten properties
  (`editor.fontFamily`/`fontSize`/`lineHeight`/`fontLigatures` from
  `font_settings`; `editor.tabWidth`/`indentSize`/`indentStyle`/
  `trimTrailingWhitespace`/`insertFinalNewline`/`wordWrap` from `import`'s
  three mapping tables) already existed as string literals scattered
  across those modules' own `match` statements. Unifying `font_settings`
  and `import` to read from this list instead of their own literals is
  real, legitimate future cleanup — deliberately *not* done in this loop:
  those five modules each have their own closed, passing receipt from
  earlier this phase, and touching already-verified code outside this
  deliverable's own scope isn't what this loop is for.

  **`completion.rs`: a small hand-written state machine, not the
  `jsonc-parser` AST this crate uses everywhere else — a real, load-bearing
  reason, not a shortcut.** Completion has to answer "what belongs at the
  cursor" on a document that's *mid-edit*, which is very often
  syntactically incomplete exactly at the cursor — an unterminated string,
  a key with no colon yet. `jsonc-parser`'s AST parser (used by
  `layering`, `keymap::presets`, and both JSON-based `import` modules) is
  built for complete, valid documents and would simply fail to parse at
  all in that state, which is the wrong tool for this specific job. The
  scanner here is deliberately narrower in exchange for that tolerance: it
  only understands a **flat, single-level object** — which is what every
  settings document in this crate actually is (nothing in `layering`,
  `import`, or `font_settings` ever nests). A nested object or array value
  would confuse it; that limitation is written into the module doc, not
  discovered later and hidden.

  **A real, documented nuance the state machine has to get right: value
  completions have to know whether they're already inside an open quote.**
  `editor.indentStyle`'s enum values (`"tab"`/`"space"`) need their
  surrounding quotes added when completing right after the colon
  (`"editor.indentStyle": ` → insert `"tab"`) but *not* when completing
  inside a string the user already opened (`"editor.indentStyle": "ta` →
  insert bare `tab`, replacing just the typed `ta`) — inserting a
  double-quoted value inside an already-open string would produce broken
  JSON. `editor.fontLigatures`'s boolean values are the mirror case: `true`/
  `false` are bare JSON literals, so they're only offered *outside* an open
  string at all — offering them inside one (`"editor.fontLigatures": "` )
  would insert a *string* `"true"`, not the boolean the key actually needs,
  so that case correctly offers nothing rather than a subtly wrong
  completion. Both directions are exercised directly
  (`offers_unquoted_enum_values_inside_an_already_open_string`,
  `does_not_offer_a_boolean_literal_inside_an_open_string`).

  **JSONC comments are handled, not just tolerated by accident.** This
  crate's entire identity is JSONC-with-comments (the very first loop of
  this phase built `layering::jsonc` around exactly that), so a completion
  engine that silently produced wrong answers the moment a `//` or `/* */`
  comment appeared before the cursor would be a real, non-obvious footgun
  undermining that identity. The scanner detects comment start outside of
  string contexts and skips their content without letting it perturb the
  key/value state machine, with the cursor landing *inside* a comment
  correctly yielding no completions at all (a real bug was caught and
  fixed here before the first test run — see below).

  **Two bugs found and fixed by tracing the logic by hand, before ever
  running the test suite — worth recording since it's a different failure
  mode than every other loop this phase, which each found their one real
  bug via a failing `cargo test` run.** (1) The boolean-value test
  initially asserted the wrong iteration order for `["true", "false"]`
  suggestions (`items[0]` is `true`, not `false` — a test-authoring
  mistake, not an implementation bug, caught by re-reading the match arm
  against the assertion before compiling). (2) A real logic bug in the
  comment-skipping helpers: when a `//` or `/* ` comment runs all the way
  to the end of the scanned text without ever finding its closing
  delimiter (a trailing, unterminated comment — exactly what "the cursor
  sits inside a comment with nothing after it" looks like), the original
  code fell out of its `while` loop without ever setting `Mode::InComment`,
  because the `stop_at`-reached check inside the loop body is never
  triggered when `stop_at` equals the *end* of the text: the last
  character processed always has an index strictly less than `text.len()`.
  The fix: treat running out of characters while still inside the
  comment's own scanning loop the same as hitting `stop_at` mid-comment —
  set `Mode::InComment` there too. Caught by tracing
  `the_cursor_inside_a_comment_offers_nothing`'s exact expected behavior
  against the code before running it, not by a failing assertion — but
  it's the same class of edge case a failing test would have caught, and
  the test stayed in the suite specifically to guard it going forward.

  **Actual verification.** `cargo test -p lux-config`: 100/100 unit tests
  passed, 0 ignored, on the very first `cargo build`/`cargo test` run after
  both fixes above — no failing run needed to find either bug. Re-ran
  three times consecutively afterward; identical results (no filesystem or
  threading involved in `schema`/`completion`'s tests — pure logic, same
  no-flakiness-risk category as the `import`/`font_settings` loops).
  `schema::tests` (3): every property key unique, `find` locating a real
  key, `find` returning `None` for an unknown one. `completion::tests`
  (16): every known key offered right after `{`; key completions filtered
  by typed prefix; an already-present key excluded from further
  suggestions; key `insert_text` is bare (no quotes/colon added — that's
  the editor's job, this only supplies the identifier); quoted vs.
  unquoted enum-value completion depending on whether a string is already
  open; bare boolean literals offered outside a string and correctly
  withheld inside one; `String`/`Integer`/`Number`-kind keys and unknown
  keys offering no value completions (open-ended or unrecognized — nothing
  fabricated); no completions between a complete value and its next comma;
  key completion resuming correctly after that comma; a line comment and a
  block comment each *not* confusing key/value detection when they precede
  the cursor; the cursor landing inside a comment offering nothing (the
  fixed bug, now guarded); and an escaped quote inside a key string not
  ending that string early. Full `cargo test -p lux-config` (no filter):
  100 unit tests (all of `schema`/`completion` plus the 81 accumulated from
  the five earlier Phase 5 loops) and 7 integration tests from
  `keymap_presets`, all green. Full repo gate also run and green: `cargo
  fmt --all --check` (after one `cargo fmt --all` pass); `cargo clippy
  --workspace --all-targets -- -D warnings` (zero warnings); `cargo test
  --workspace` (all crates, no regressions — `lux-core`'s 255,
  `lux-workspace`'s 54, everything else unchanged); `cargo deny check`
  (advisories/bans/licenses/sources all `ok` — no new dependency was added
  this loop, `schema`/`completion` are pure logic over the standard
  library); `scripts/check-limits.sh` (`completion.rs` at 510 lines, the
  largest file this loop added, under the 600-line cap; no struct exceeds
  4 fields); `scripts/check-core-isolation.sh` (unaffected — only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only, same
  documented limitation as the rest of `docs/STATUS.md`.

  **What's out of scope and untouched — including one thing worth being
  explicit about.** Any UI actually rendering a completion popup (no
  `lux-ui`/`lux` wiring, per the choice documented above); hover/
  documentation-on-demand for a property (a natural companion feature,
  not asked for here); unifying `font_settings`/`import`'s own key
  literals to read from `schema::PROPERTIES` instead of duplicating them
  (real cleanup, deliberately deferred — see above). **Phase 5 deliverable
  1's still-outstanding gap — "esquema publicado y validación con errores
  útiles señalando línea y columna" — is related but was not closed by
  this loop and should not be assumed closed by it.** `schema.rs` is a
  real schema, but it's shaped for *suggesting* values at a cursor
  position, not for *validating* an already-complete document end-to-end
  and reporting every mismatch with a line and column — that's a different
  algorithm (walk a fully-parsed document, not scan tolerantly toward one
  cursor position) that would need its own loop. `/close-phase 5` should
  still treat that piece as open until a receipt exists for it.
