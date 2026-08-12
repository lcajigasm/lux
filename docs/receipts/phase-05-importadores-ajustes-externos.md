# Receipt: Importadores de ajustes externos (deliverable 4 of Phase 5)

- Phase: 05
- Crate(s): lux-config
- Goal: The "Importadores de ajustes externos" deliverable of Phase 5
  ("Configuración, keymaps y personalización") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-config import::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** Phase 5's deliverable 4 text: "Importadores de ajustes de
  Sublime, VS Code y JetBrains **conscientes del esquema**, que informen
  honestamente de qué se importó y qué se ignoró." Three importers, all in
  scope for this loop. `docs/STATUS.md`'s "Settings importers" row stays
  `not-started` (rule 1), same pattern as every loop this phase: nothing
  consumes an `ImportReport` yet — no command, no UI, no write path into
  the `layering::LayeredConfig` two loops ago built — and nothing is wired
  into the `lux` binary.

  **"Schema-aware" against a real, if informal, schema.** Lux doesn't have
  a published settings schema yet (Phase 5 deliverable 1's schema
  publication is explicitly deferred, separate work). Rather than block on
  that or invent a formal schema document just to satisfy one word in this
  deliverable's text, each importer maps onto a small, real set of dotted
  Lux setting keys (`editor.tabWidth`, `editor.indentStyle`,
  `editor.indentSize`, `editor.trimTrailingWhitespace`,
  `editor.insertFinalNewline`, `editor.wordWrap`, `editor.fontFamily`,
  `editor.fontSize`, `editor.lineHeight`) — the same dotted-key shape
  `layering`'s JSON-object settings already use, so a future integration
  writing an `ImportReport`'s output into a real `LayeredConfig` layer is a
  straight `serde_json::Map` insert, not a translation step. "Schema-aware"
  here means what actually matters for the deliverable's real ask: an
  importer that knows what it's looking for and reports precisely, not one
  that blindly copies every foreign key into Lux's settings verbatim.

  **The honesty requirement produced a real three-way outcome, not just
  imported-vs-ignored.** `IgnoreReason` has two variants:
  `UnknownKey` (Lux has no equivalent at all) and
  `UnsupportedValue { expected }` (the key is recognized, but its value
  wasn't the type Lux expects — e.g. VS Code's `editor.wordWrap` set to
  `"wordWrapColumn"`, a real value this crate's boolean word-wrap toggle
  genuinely can't represent yet). Collapsing both into one generic
  "ignored" would have technically satisfied "qué se ignoró" but lost the
  more useful distinction a real user-facing import dialog would want:
  "we don't know this setting" reads differently from "we know this
  setting but couldn't use the value you gave it." Every source key
  present in an input file lands in exactly one of `imported`/`ignored` —
  tested directly (`every_key_lands_in_exactly_one_list` in
  `import::sublime`).

  **Sublime and VS Code: parsed with the JSONC parser `layering` already
  built, not a second one.** Both are JSON-with-comments formats (Sublime's
  `Preferences.sublime-settings`, VS Code's `settings.json`); both
  importers call `layering::jsonc::parse` directly — a real reuse of
  existing crate infrastructure, not a new parsing path. VS Code's own
  settings are already flat dotted keys (`"editor.tabSize"`, not a nested
  object), so no flattening step was needed before mapping either.

  **JetBrains: a new dependency, `quick-xml`, because nothing else in the
  crate parses XML.** JetBrains' code-style settings
  (`.idea/codeStyles/Project.xml`'s `<indentOptions>` block) are real XML —
  the one source format `jsonc-parser`, `ec4rs`, and `gix-attributes`
  (this phase's three prior dependency additions) don't cover. `quick-xml`
  0.41.0 (MIT) is a real streaming reader; hand-rolling attribute/quoting/
  escaping parsing for even this narrow a slice of XML would duplicate
  real, tricky, already-solved work. `import::jetbrains::import` does a
  flat scan for every `<option name="..." value="...">` element anywhere
  in the document, regardless of nesting — a documented, deliberate
  simplification: a DOM-aware reader that only looked inside
  `<indentOptions>` would be more precise about *where* an option came
  from, but the deliverable's own wording only asks that imports honestly
  report what was recognized versus not, which a flat scan already
  satisfies (every `<option>` found is either imported or explicitly
  ignored, never silently dropped for being in an unexpected place).

  **Every mapped source key: verified against something real, and that
  verification's strength (or its absence) is written down rather than
  presented as uniform.** VS Code's `editor.tabSize`, `editor.insertSpaces`,
  and `editor.wordWrap` were confirmed directly against
  `code.visualstudio.com`'s own documentation (fetched this loop).
  JetBrains' `TAB_SIZE`, `INDENT_SIZE`, and `USE_TAB_CHARACTER` were
  confirmed against the strongest possible evidence — read straight from
  `CommonCodeStyleSettings.java`'s `IndentOptions` class in the
  `intellij-community` GitHub repository, the actual Java source that
  produces this exact XML, rather than assumed from memory. A fourth
  JetBrains field, an "insert final newline" equivalent, was considered
  and deliberately dropped: unlike the three that were confirmed, its
  exact XML option name wasn't something this loop could verify with
  confidence, and a wrong literal name would either silently fail to match
  real files or (worse) collide with an unrelated real option — the same
  "don't fabricate a shortcut" discipline the keymap-preset loop applied
  to Sublime's bindings. Sublime's `font_face`, `tab_size`, and `word_wrap`
  were confirmed against `sublimetext.com/docs/settings.html` (the one
  place Sublime's own docs name concrete keys in prose); its remaining
  keys (`translate_tabs_to_spaces`, `trim_trailing_white_space_on_save`,
  `ensure_newline_at_eof_on_save`, `font_size`) and VS Code's
  `files.trimTrailingWhitespace`, `files.insertFinalNewline`,
  `editor.fontFamily`, `editor.fontSize`, `editor.lineHeight` come from
  well-established, long-stable prior knowledge instead — each importer
  module's doc comment says exactly which of its own keys fall into which
  category, rather than a single blanket "verified" claim covering all of
  them.

  **Actual verification.** `cargo test -p lux-config import::`: 20/20
  passed, 0 ignored, on the first run — no fix-and-retry cycle needed this
  loop (the two prior `.editorconfig`/`.gitattributes` and `layering` loops
  each hit one real bug; this one didn't). Re-ran three times consecutively
  afterward as a sanity check; identical results each time (no filesystem
  or threading involved in any of these tests — pure parsing/mapping
  logic, so no flakiness risk the way the hot-reload loop had). Coverage
  per importer (`import::sublime`, `import::vscode`, `import::jetbrains`,
  ~6-7 tests each): known keys mapping to the right Lux key and value;
  boolean-inversion cases (`translate_tabs_to_spaces`/`USE_TAB_CHARACTER`
  false mapping to `"tab"`, not silently defaulting to `"space"`); a
  source-specific quirk each format actually has (Sublime's `word_wrap`
  accepting `"auto"` as well as a plain boolean; VS Code's `wordWrapColumn`
  value correctly reported as unsupported rather than coerced to `true`/
  `false`; JetBrains options found at arbitrary nesting depth); an
  unrecognized key reported as `UnknownKey`; a recognized key with the
  wrong value type reported as `UnsupportedValue` with the right expected-
  type string; every key landing in exactly one list; and a real parse
  error (malformed JSONC / malformed XML) surfacing as a real `Err`, not a
  panic or a silently-empty report. Full `cargo test -p lux-config` (no
  filter): 74 unit tests (the 20 above plus 54 from the three earlier Phase
  5 loops) and 7 integration tests from `keymap_presets`, all green. Full
  repo gate also run and green: `cargo fmt --all --check` (after one
  `cargo fmt --all` pass); `cargo clippy --workspace --all-targets -- -D
  warnings` (zero warnings — including fixing one *deprecation* warning
  along the way: `quick-xml`'s `Attribute::unescape_value` is deprecated in
  favor of `normalized_value`, which needs an XML-version parameter this
  narrow use case doesn't need; switched to a plain lossy UTF-8 decode of
  the raw attribute bytes instead, correct here since the only attribute
  values this module ever reads are plain identifiers/numbers/booleans,
  never XML entities); `cargo test --workspace` (all crates, no
  regressions — `lux-core`'s 255, `lux-workspace`'s 54, everything else
  unchanged); `cargo deny check` (advisories/bans/licenses/sources all
  `ok`, no new `deny.toml` entries needed — `quick-xml` is MIT, already
  allowed); `scripts/check-limits.sh` (`import/jetbrains.rs` at 166 lines,
  the largest new file, well under the 600-line cap; no struct exceeds 3
  fields); `scripts/check-core-isolation.sh` (unaffected — only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only, same
  documented limitation as the rest of `docs/STATUS.md`.

  **What's out of scope and untouched:** a preferences surface or any
  actual "Import Settings" command/UI (Phase 5 deliverable 6, still
  not-started); writing an `ImportReport`'s `imported` settings into a
  real `LayeredConfig` layer — this deliverable produces the report,
  consuming it is a future integration's job, same design language
  `layering`/`keymap`/`editorconfig`/`gitattributes` already established
  this phase; import of any setting beyond the handful of indent/
  whitespace/font ones each importer currently recognizes (keymaps,
  themes, extensions/plugins, and everything else in a real Sublime/VS
  Code/JetBrains settings export); JetBrains' many *other* real settings
  file formats (its full XML settings surface goes far beyond
  `<indentOptions>` — this loop only mapped what could be confirmed
  against real source); conflict handling if an imported setting would
  collide with an existing user setting (a future integration's job, once
  something actually writes imports into a live config).
