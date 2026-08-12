# Receipt: Esquema publicado + validación con línea/columna (closing the last gap of deliverable 1)

- Phase: 05
- Crate(s): lux-config
- Goal: Terminar lo pendiente para cerrar la Fase 5 — specifically, the
  last outstanding piece of deliverable 1 ("lux-config: ... esquema
  publicado y validación con errores útiles señalando línea y columna"),
  flagged as open by `/close-phase 5`'s first attempt and by the
  deliverable-6 receipt's closing note. Its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-config layering::validate::`
- Iterations: 1 (no cap was set for this open-ended `/goal`; the first
  build-and-test run passed all 12 new tests with no fix-and-retry cycle)
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Why this loop exists.** `/close-phase 5` was attempted and stopped at
  its own step 2 (receipt-completeness check): five of six deliverables
  had a passing receipt, but deliverable 1's own receipt explicitly
  flagged that its "esquema publicado y validación con errores útiles
  señalando línea y columna" clause was deferred, and deliverable 6 (built
  afterward) needed `crate::schema` to exist before validation could be
  built on top of it — so the two gaps were really one gap, closeable only
  once both deliverable 1's original loop and deliverable 6's loop had
  landed. This loop closes it.

  **Two real pieces, not one: "esquema publicado" and "validación con
  línea/columna" are related but distinct, and both needed to actually
  exist.** `schema::PROPERTIES` (from the deliverable-6 loop) is Lux's own
  in-process vocabulary — real, but not "published" in the sense an
  external tool could consume without linking against this crate.
  `schema::to_json_schema()` closes that: it exports the same ten
  properties as a real [JSON Schema](https://json-schema.org/) document
  (draft-07 — `type`, `properties`, per-property `type`/`enum`/
  `description`, `additionalProperties: false`), the standard format a
  real editor's `$schema` reference, a future language server, or any
  JSON-Schema-aware tool already knows how to read — not a Lux-specific
  shape only this crate could parse. `layering::validate` is the second,
  independent piece: this crate's *own* validator, which doesn't consume
  `to_json_schema()`'s output at runtime (the two agree because both read
  from the same `schema::PROPERTIES` list, not because one depends on the
  other) — a real, load-bearing choice: a general external JSON Schema
  *validator* library would be a much larger dependency for what a dozen
  lines of matching against an already-in-process schema does directly.

  **`layering::validate` deliberately does *not* reuse `completion`'s
  tolerant scanner, even though both are JSONC-position-aware — a real
  design decision, not an oversight.** `completion` exists specifically to
  answer "what belongs here" on a document that's mid-edit and often
  syntactically incomplete at the cursor; `jsonc_parser`'s real AST parser
  would simply fail on that input. Validation is the opposite case: it's
  only meaningful once a document is syntactically *complete* — there's
  nothing coherent to say about "is this value the right type" for a
  document that isn't finished being typed. So `validate` uses
  `jsonc_parser::parse_to_ast` directly (the same crate `layering::jsonc`
  and `keymap::presets` already depend on, no new dependency), and a
  syntactically invalid document short-circuits to exactly one
  diagnostic — the parser's own `ParseError`, which already carries a real
  line and column via its public `line_display()`/`column_display()`
  methods — rather than every schema rule silently not running against a
  tree that was never built.

  **Line/column for AST-derived diagnostics (unknown keys, type
  mismatches) needed its own small implementation, not a shortcut through
  private crate internals.** `jsonc-parser`'s own line/column conversion
  (`get_line_and_column_display`, used internally to build `ParseError`'s
  message) is a private function, not part of its public API — confirmed
  by reading `errors.rs` directly rather than assuming. `layering::validate`
  reimplements the same small, well-defined algorithm independently (count
  `\n`s and `char`s up to a byte offset, 1-based line and column) rather
  than reaching for something not actually exposed. The two sources of
  diagnostics — `jsonc-parser`'s own parse errors and this module's schema
  checks — end up using the identical convention on purpose, so a caller
  displaying both kinds of diagnostic together doesn't have to reconcile
  two different position schemes.

  **What counts as a "diagnostic" here, concretely.** Three checks, one
  finding per property: (1) a key with no matching `schema::find` entry —
  reported at the *key's* position, since that's what a user reading the
  file would need to look at; (2) a value whose JSON type doesn't match
  the property's `PropertyKind` (a string where a boolean was expected,
  etc.) — reported at the *value's* position; (3) two refinements beyond a
  bare type check that genuinely needed the schema, not just "is this
  JSON": an `Enum` property's string value checked against its actual
  allowed set (`"editor.indentStyle": "spaces"` is a syntactically valid
  string but not one of `"tab"`/`"space"` — the error names both invalid
  and valid values), and an `Integer` property whose `NumberLit` contains
  a literal `.` reported as "expected an integer, found the fractional
  number ..." rather than silently accepted as if `Integer` and `Number`
  were the same thing (they're the same *behavior* in `completion`, which
  has no discrete integer-vs-fractional values to offer either way, but
  they're not the same *validation rule* — this is exactly the distinction
  `schema.rs`'s own doc comment flagged as "distinct... even though the
  two behave identically in `completion` today").

  **Actual verification.** `cargo test -p lux-config layering::validate::`:
  12/12 passed, 0 ignored, on the first `cargo build`/`cargo test` run — no
  failing run needed. Coverage: a well-formed document producing zero
  diagnostics; an unknown key reported at its own (verified, hand-counted)
  line and column; a type mismatch reported at the value's own (also
  hand-counted) position — both position assertions computed by manually
  counting characters in the test's literal source text rather than just
  asserting "some positive number," so a future regression that shifts the
  position by even one column would actually fail the test; a fractional
  number correctly flagged for an `Integer` key and correctly accepted for
  an actual integer; an invalid enum value's message naming the real valid
  options, and a valid one producing no diagnostic; multiple independent
  problems in one document all reported, not just the first (a validator
  that stops at the first error is a worse tool than one that doesn't);
  malformed JSONC syntax collapsing to exactly one diagnostic with a real
  position; an empty document and a top-level non-object (an array) each
  producing one clear, specific diagnostic instead of a panic or a
  misleading "no problems found"; and a comment appearing before an
  offending value not throwing off that value's reported line (JSONC's
  defining feature staying correctly handled here too, the same concern
  `completion`'s own comment-awareness tests already covered for a
  different module). `schema::tests` gained 3 more (3 → 6 total, on top of
  the deliverable-6 loop's original 3): every schema property present in
  the generated JSON Schema; an `Enum`
  property represented with both `type: "string"` and its real `enum`
  list; `additionalProperties: false` present (an unrecognized key should
  be rejected by an external JSON-Schema-aware tool too, not just by this
  crate's own validator). Full `cargo test -p lux-config` (no filter): 115
  unit tests (12 + 3 new, plus the 100 accumulated from the five earlier
  Phase 5 loops) and 7 integration tests from `keymap_presets`, all green.
  Full repo gate also run and green: `cargo fmt --all --check` (after one
  `cargo fmt --all` pass); `cargo clippy --workspace --all-targets -- -D
  warnings` (zero warnings); `cargo test --workspace` (all crates, no
  regressions — `lux-core`'s 255, `lux-workspace`'s 54, everything else
  unchanged); `cargo deny check` (advisories/bans/licenses/sources all
  `ok` — no new dependency added this loop, both new pieces build on
  `jsonc-parser` and `serde_json`, already-present dependencies);
  `scripts/check-limits.sh` (`validate.rs` at 276 lines and `schema.rs` at
  196, both well under the 600-line cap); `scripts/check-core-isolation.sh`
  (unaffected — only checks `lux-core`). Verified on macOS
  (aarch64-apple-darwin) only, same documented limitation as the rest of
  `docs/STATUS.md`.

  **What's out of scope and untouched:** any UI surfacing these
  diagnostics (an editor's problem panel, inline squiggles — no
  `lux-ui`/`lux` wiring, consistent with every other Phase 5 module);
  validating the *merged* view a `LayeredConfig` produces across all five
  layers at once — `validate` operates on one document's text, matching
  `completion`'s own single-document scope, since layer-merge validation
  (which layer contributed the invalid value?) is a materially different
  problem this loop doesn't attempt; consuming `to_json_schema()`'s output
  from any real external tool (no `$schema` reference wired into the
  preset `.jsonc` files this crate ships, no exported static schema file
  on disk — the function exists and is tested, using it from outside this
  crate's own test suite is a future integration's job).

  **This loop is the last piece of Phase 5.** All six deliverables named
  in `LUX_PROMPT_MAESTRO.md`'s Phase 5 section now have a `docs/receipts/
  phase-05-*.md` file with `Result: passed`. `/close-phase 5` should be
  re-run after this receipt lands to run the full closing gate, update
  `docs/STATUS.md`'s summary, draft release notes, and propose the
  `v0.4.0` tag.
