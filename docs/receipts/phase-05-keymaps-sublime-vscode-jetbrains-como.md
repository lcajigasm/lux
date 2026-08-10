# Receipt: Keymaps (Sublime/VS Code/JetBrains) como datos (deliverable 2 of Phase 5)

- Phase: 05
- Crate(s): lux-config
- Goal: The "Keymaps (Sublime/VSCode/JetBrains) como datos" deliverable of
  Phase 5 ("Configuración, keymaps y personalización") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-config keymap:: --test keymap_presets`
- Iterations: 2 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** Phase 5's deliverable 2 text: "Keymaps como datos, con presets
  Sublime, VS Code y JetBrains, resolución de conflictos con diagnóstico
  visible, keymaps por plataforma y encadenamientos de teclas." All four
  clauses are in scope for this loop (unlike the previous `lux-config`
  loop, which explicitly narrowed deliverable 1). Not in scope: conflict
  *resolution policy* (deciding which of two conflicting bindings wins, or
  prompting the user) — the deliverable asks for detection with a visible
  diagnostic, not an auto-resolver; and actually dispatching a key event to
  a command, since no input/keystroke layer exists yet for this to hook
  into. `docs/STATUS.md`'s "Keymap presets" row stays `not-started` (rule
  1), same pattern as every other Phase 4/5 loop so far: a headless,
  verified library with nothing wired into the `lux` binary.

  **Data model: bindings are literally data, not Rust code.** Each preset
  is a checked-in `.jsonc` file (`crates/lux-config/keymaps/{sublime,
  vscode,jetbrains}.jsonc`, not `src/`, to make that distinction visible in
  the repo layout), parsed at runtime through the same `layering::jsonc`
  parser the previous `lux-config` loop built (`serde` feature of
  `jsonc-parser`) — a direct, cheap reuse of already-landed infrastructure,
  not a second bespoke JSONC path. `KeyBinding`'s shape
  (`command`/`key`/`mac`/`win`/`linux`/`when`) mirrors VS Code's real
  `keybindings.json` schema on purpose: a default `key` for Windows/Linux,
  optionally overridden per platform, `when` carried through as an opaque
  context string. That's genuine prior art for "keymaps por plataforma",
  not an invented format, and it's general enough that Sublime's and
  JetBrains' presets are expressed through the exact same shape with no
  VS-Code-specific fields.

  **Chords parse from the same string convention VS Code's own keymap
  files use** — `"+"`-joined modifiers ending in a key
  (`"ctrl+shift+p"`), chords in a sequence separated by whitespace
  (`"ctrl+k ctrl+s"`). Reusing that exact real convention for
  `ChordSequence`'s `FromStr`/`Deserialize` is what makes "encadenamientos
  de teclas" fall out of the string format directly — a chained binding is
  just a string with a space in it, not a second binding type or a nested
  array field. `Chord` itself is boolean modifier flags plus a lowercased
  key string, with derived `PartialEq`/`Hash` — parsing normalizes case and
  modifier order before storing, so `"Ctrl+Shift+P"` and `"shift+ctrl+p"`
  compare equal.

  **Conflict detection is deliberately narrow about what counts as a
  conflict.** Two bindings only get flagged when they resolve to the same
  chord sequence on the same platform *and* have identical `when` clauses
  (including both being unset) — genuinely indistinguishable at runtime.
  Building a real `when`-expression evaluator to detect *semantic* overlap
  between different context strings (e.g. recognizing `"editorFocus"` and
  `"editorTextFocus"` might overlap) is a much bigger feature the
  deliverable's text doesn't ask for ("resolución de conflictos con
  diagnóstico visible", not a `when`-clause engine) — `when` is carried as
  an opaque string throughout, documented in `conflicts.rs` as a deliberate
  simplification rather than an oversight. `Conflict` implements `Display`
  ("`ctrl+d` is bound to multiple commands: duplicate_line,
  select_next_occurrence") — a real, surfaceable diagnostic a caller could
  put in a UI, not just an internal `bool`.

  **Preset shortcuts: sourced from official docs where they were
  reachable, from well-established prior knowledge where they weren't —
  documented as such rather than presented as uniformly verified.** Fetched
  VS Code's (`code.visualstudio.com`) and JetBrains' (`jetbrains.com/help`,
  both the Windows/Linux and macOS default-keymap reference pages)
  official documentation directly and cross-checked every VS Code/JetBrains
  shortcut in the presets against what those pages actually state,
  including two real macOS divergences a naive Ctrl-to-Cmd substitution
  would have gotten wrong (JetBrains' "Go to File" is `Cmd+Shift+O` on mac,
  not `Cmd+Shift+N`; its "Select Next Occurrence" stays `Ctrl+G` on mac,
  not `Cmd+G`, because `Cmd+G` is already "Find Next") — both encoded and
  commented inline in `jetbrains.jsonc`. Sublime Text has no equivalent
  plain-text official reference (its docs point at the shipped `Default
  ({Platform}).sublime-keymap` package files, not fetchable here); its
  preset's shortcuts come from well-established, long-stable prior
  knowledge instead, flagged as such in a comment at the top of
  `sublime.jsonc` rather than silently presented as fetched-and-confirmed.
  Each preset covers a representative, modest subset (11-12 bindings), not
  every default — matching the phase's own acceptance criterion ("los tres
  presets... pasan un test que verifica los atajos documentados") without
  claiming exhaustive coverage the master prompt doesn't ask for.

  **A real modifier-order bug, caught by the first test run, not a data
  error.** The first `cargo test -p lux-config keymap:: --test
  keymap_presets` run failed 3 of 7 tests (Sublime, VS Code, and
  JetBrains' `command_palette`/`find_action` assertions), all with the same
  shape: `left: "shift+cmd+p"`, `right: "cmd+shift+p"`. Root cause: the
  test helper compared `Chord::Display`'s fixed canonical rendering order
  (`ctrl, alt, shift, cmd`) against hand-written expected strings that
  followed each vendor's own documented order (`cmd+shift+p`, matching how
  VS Code and Sublime actually write it) — and real editor docs don't
  agree on modifier order across every command anyway (VS Code writes
  "Shift+Alt+Down" for one shortcut but "Ctrl+Shift+P" for another, so no
  single fixed order matches all of them). The data itself was correct;
  the test was comparing the wrong representation. Fixed by making
  `assert_binding` parse the expected string into a `ChordSequence` and
  compare it structurally (`Chord`'s modifiers are boolean flags, not an
  ordered sequence — structural equality is what "matches" should mean),
  not by reordering `Display` to chase one convention that wouldn't have
  matched them all anyway. Re-ran clean on the next attempt, and three more
  times afterward with identical results — this failure had no timing
  component (no filesystem, no threads), so repeat-run verification was a
  quick sanity check, not a flakiness hunt like the hot-reload loop before
  it.

  **Actual verification.** `cargo test -p lux-config keymap:: --test
  keymap_presets`: 7/7 passed, 0 ignored — 3 real failures (the bug above)
  on iteration 1, fixed on iteration 2, green on that run and three
  subsequent re-runs. Coverage: each preset's documented shortcuts on both
  Windows and mac, including the two real macOS divergences noted above
  (`vscode_preset_matches_its_documented_shortcuts`,
  `sublime_preset_matches_its_documented_shortcuts`,
  `jetbrains_preset_matches_its_documented_shortcuts`); two genuine
  two-chord sequences from VS Code's real default keymap
  (`vscode_preset_has_documented_two_chord_sequences`, asserting the
  resolved sequence is actually length 2, not just checking the rendered
  string); every binding across all three presets falling back correctly
  to its Windows chord on Linux when no explicit override exists
  (`linux_falls_back_to_the_windows_binding_when_no_linux_override_exists`);
  none of the three shipped presets having an accidental internal conflict
  on any platform
  (`none_of_the_three_shipped_presets_have_an_internal_conflict` — a real
  sanity check against the actual data, not a synthetic one); and a
  deliberately constructed conflicting keymap producing a diagnostic that
  names the chord and both competing commands
  (`a_real_conflict_is_detected_with_a_readable_diagnostic`). `cargo test -p
  lux-config` (no filter) also run: 36 unit tests across
  `keymap::chord`/`keymap::binding`/`keymap::conflicts`/`keymap::presets`
  plus the 19 from the previous `layering` loop, all green — chord parsing
  edge cases (case/order-insensitivity, bare keys with no modifiers,
  rejecting an unknown modifier or a trailing `+`, an empty sequence),
  platform-resolution fallback, and conflict-detection logic (same-chord
  same-context conflicts, different-`when` non-conflicts,
  different-platform-override conflicts checked independently per
  platform). Full repo gate also run and green: `cargo fmt --all --check`
  (after one `cargo fmt --all` pass); `cargo clippy --workspace
  --all-targets -- -D warnings` (zero warnings, including on the new direct
  `serde` dependency); `cargo test --workspace` (all crates, no
  regressions — `lux-core`'s 255, `lux-workspace`'s 54, `lux-config`'s now
  43 unit + 7 + 7 integration, everything else unchanged); `cargo deny
  check` (advisories/bans/licenses/sources all `ok`, no new `deny.toml`
  entries needed — `serde` was already MIT/Apache-2.0-allowed);
  `scripts/check-limits.sh` (`keymap/chord.rs` at 202 lines and
  `tests/keymap_presets.rs` at 208, both well under the 600-line cap; no
  struct exceeds 6 fields); `scripts/check-core-isolation.sh` (unaffected —
  only checks `lux-core`). Verified on macOS (aarch64-apple-darwin) only,
  same documented limitation as the rest of `docs/STATUS.md`.

  **What's out of scope and untouched:** `.editorconfig`/`.gitattributes`
  support, settings importers, typography settings, and a preferences
  surface (Phase 5 deliverables 3-6); conflict resolution *policy* (only
  detection-with-diagnostic is in scope, per the deliverable's own
  wording); actually dispatching a resolved chord sequence to a command —
  no input/keystroke-handling layer exists in the codebase yet for this to
  attach to; a `when`-clause expression language (deliberately kept
  opaque, see above); user-customizable keymaps or a merge with the
  layered settings model from the previous loop (these presets are static,
  read-only data — nothing here writes to or reads from a user's own
  keymap file); wiring any of this into the `lux` binary or any UI.
