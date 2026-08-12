# Receipt: `.editorconfig` / `.gitattributes` (deliverable 3 of Phase 5)

- Phase: 05
- Crate(s): lux-config
- Goal: The ".editorconfig / .gitattributes" deliverable of Phase 5
  ("Configuración, keymaps y personalización") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-config editorconfig::`
- Iterations: 2 / 6
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** Phase 5's deliverable 3 text: "Soporte completo de
  `.editorconfig` (globs, secciones, herencia) y de `.gitattributes` para
  normalización de finales de línea." Both halves are named in the
  deliverable's own title, so both are in scope for this loop — unlike the
  narrowed `layering` loop earlier in this phase. The `.gitattributes` half
  is explicitly narrowed by the deliverable's own wording ("para
  normalización de finales de línea"): diff/merge drivers, filters,
  `export-ignore`, and `linguist-*` hints are real `.gitattributes`
  features but outside what this bullet asks for. The verify command only
  gates `editorconfig::`, but `.gitattributes` was still implemented and
  tested to the same standard (9 tests of its own,
  `cargo test -p lux-config gitattributes::`) — not left half-done just
  because it isn't the named stop condition. `docs/STATUS.md`'s
  "`.editorconfig` / `.gitattributes`" row stays `not-started` (rule 1),
  same pattern as every other loop this phase: nothing in `lux-core`'s
  buffer/save path consults either module yet, and nothing is wired into
  the `lux` binary.

  **`.editorconfig`: a dedicated spec-compliant crate (`ec4rs` 1.2.0), not
  a hand-rolled implementation.** EditorConfig's glob dialect (character
  classes, `{a,b}` brace alternation, `{1..10}` numeric ranges) is its own
  grammar — different enough from gitignore-style globs that the
  `ignore`/`globset` family already in this workspace (Phase 4) doesn't
  actually cover it. The master prompt's own wording, "soporte **completo**
  de `.editorconfig`", pushed toward a real, spec-tested implementation
  over a partial hand-rolled one that would likely miss an edge case (a
  half-correct glob matcher would be worse than none — it would look like
  glob support works when it silently doesn't cover some case, the same
  reasoning `lux-workspace`'s Phase 4 receipt gave for *not* implementing
  its own `excludes` matching). `ec4rs` does the real work this deliverable
  asks for — directory-tree walk-up with `root = true` stopping it, `[glob]`
  section matching, later-declared-and-closer-file-wins merging — and
  `lux_config::editorconfig` is a thin typed wrapper around it
  (`ec4rs::properties_of` → a typed `EditorConfig` struct using `ec4rs`'s
  own property enums directly, rather than inventing parallel types for
  `indent_style`/`indent_size`/etc.). Apache-2.0, added to `deny.toml`'s
  allow list with no new entry needed (already permissive-covered).

  **`.gitattributes`: `gix-attributes` 0.34.0 — the same gitoxide family
  `docs/adr/0010-git-backend-gix.md` already committed to for Phase 6, used
  here first, ahead of that phase, purely for its pattern-matching
  engine.** `lux-git` itself has no `gix` dependency yet (Phase 6 hasn't
  started), so this is the first gitoxide-family crate actually wired into
  the workspace — deliberately the *current* `gix-attributes` line, not the
  archived standalone `git-attributes` 0.8.3 crate from gitoxide's early
  pre-split days, which was tried first via `cargo add` and then removed
  once its age (and the outdated `git-features`/`git-hash`/`git-glob`
  transitive chain it would have dragged in, disconnected from `gix`
  0.86.0's actual dependency epoch) became clear from reading its
  `Cargo.toml` rather than just trusting the crate name. `gix-attributes`
  gives real gitignore-style glob matching and real attribute-assignment
  precedence (a later-added pattern list overrides an earlier one for the
  same attribute — confirmed by reading `Search::pattern_matching_relative_path`'s
  source, which iterates added pattern lists in reverse) for free;
  `lux_config::gitattributes::GitAttributes` is what narrows that general
  engine to this deliverable's actual ask: resolving `text`/`eol` state
  into an `EolNormalization` enum
  (`Normalize(Lf|CrLf)`/`Auto`/`Disabled`/`Unspecified`). The built-in
  `binary` macro (`[attr]binary -diff -merge -text`, the same one real Git
  always defines) is pre-registered in `GitAttributes::new` so a
  `.gitattributes` file can say `*.png binary` the way a real one would,
  without every file needing to spell out `-text` by hand. Pulled in 12
  further `gix-*`/`tinyvec`/`unicode-*` crates and `bstr` (pinned to
  1.13.1, matching the version this dependency chain already resolves to
  rather than letting two versions coexist) — all MIT/Apache-2.0, no new
  `deny.toml` entries needed (`unicode-normalization`'s Unicode data tables
  are covered by the `Unicode-3.0` entry Phase 2 already added).

  **A real leniency in `ec4rs` that changed what "malformed file" needed to
  mean for the test, found by reading its source rather than guessing.**
  The first attempt at a malformed-`.editorconfig` test used an unterminated
  section header (`[*.rs` with no closing bracket) as the very first
  non-root line — and `resolve()` returned `Ok` with every field unset
  instead of the expected `Err`. Reading `ec4rs::file::ConfigFiles::open`
  explained why: its directory-walk loop does
  `if let Ok(file) = ConfigFile::open(dir.join(filename)) { ... }` — a file
  that fails to open (which includes failing to parse its *preamble*, the
  `root = true`-or-not lines before the first section header) is silently
  treated as "this file doesn't apply" and the walk just continues upward,
  never surfacing an error. A bad line *inside* an already-validly-opened
  section, by contrast, *does* propagate through `ConfigFile::apply_to` as
  a real `Error::InFile`. Fixed by moving the malformed line into a section
  body (`root = true\n[*.rs]\nindent_style = \n` — a key with no value,
  which `ec4rs`'s line parser rejects unless the `allow-empty-values`
  feature is enabled, which this crate doesn't use) — a data-driven
  correction to the test's premise, not a workaround for a bug in `ec4rs`
  itself. Re-ran the full `editorconfig::` suite three times afterward;
  identical results each time (no timing component to this failure — pure
  parsing logic, not filesystem-event timing like the earlier `layering`
  loop's flake).

  **Actual verification.** `cargo test -p lux-config editorconfig::`: 9/9
  passed, 0 ignored — 1 real test-premise error (the `ec4rs` leniency
  above) on iteration 1, fixed on iteration 2, green on that run and three
  subsequent re-runs. Coverage: a matching glob section resolving
  `indent_style`/`indent_size`; a non-matching section contributing
  nothing; a closer `.editorconfig` overriding one property while a
  sibling property still inherits from the outer file (real merge, not
  wholesale replacement); `root = true` actually stopping the upward walk
  (verified by an outer file whose setting would leak through and produce
  the wrong answer if `root` were ignored); `{js,ts}`-style brace
  alternation matching every listed extension and correctly excluding
  others; `indent_size = tab` resolving to a distinct `UseTabWidth` value
  from a numeric size; boolean properties parsing to real `bool`s; a
  malformed file surfacing a real, path-naming error; and no
  `.editorconfig` anywhere in the chain resolving to every field `None`
  (noted as depending on this dev machine genuinely having no stray
  `.editorconfig` above its temp directory, same category of environment
  assumption as the rest of this phase's tests). `cargo test -p lux-config
  gitattributes::` (not the named gate, but run and verified to the same
  standard): 9/9 passed — `eol=lf`/`eol=crlf` resolving to the matching
  `LineEnding`, plain `text` with no `eol` resolving to `Normalize(Lf)`
  (Git always normalizes repository storage to LF regardless of platform),
  `text=auto` resolving to `Auto`, explicit `-text` and the `binary` macro
  both resolving to `Disabled`, a contradictory `-text eol=lf` line
  correctly letting `-text` win, an unmatched path resolving to
  `Unspecified`, and a later-added file's rule overriding an earlier one's
  for the same pattern. Full `cargo test -p lux-config` (no filter): 54
  unit tests (the 9+9 above plus the 36 from the `keymap` loop) and 7
  integration tests from `keymap_presets`, all green. Full repo gate also
  run and green: `cargo fmt --all --check` (clean on the first pass, no
  reformatting needed this loop); `cargo clippy --workspace --all-targets
  -- -D warnings` (zero warnings, including on the four new dependencies
  and their transitive chain); `cargo test --workspace` (all crates, no
  regressions — `lux-core`'s 255, `lux-workspace`'s 54, everything else
  unchanged); `cargo deny check` (advisories/bans/licenses/sources all
  `ok` — the only output is the pre-existing `windows_x86_64_*`/`bitflags`
  duplicate-version *warnings* from `lux-platform`'s `winit` stack,
  unrelated to this change and already below `deny.toml`'s
  `multiple-versions = "warn"` threshold, not a failure); `scripts/check-limits.sh`
  (`editorconfig.rs` at 264 lines, `gitattributes.rs` at 236, both well
  under the 600-line cap; no struct exceeds 8 fields);
  `scripts/check-core-isolation.sh` (unaffected — only checks `lux-core`).
  Verified on macOS (aarch64-apple-darwin) only, same documented limitation
  as the rest of `docs/STATUS.md`.

  **What's out of scope and untouched:** settings importers, typography
  settings, and a preferences surface (Phase 5 deliverables 4-6); full
  `.gitattributes` semantics beyond line-ending normalization (diff/merge
  drivers, filters, `export-ignore`, `linguist-*`, per the deliverable's
  own narrowed wording); discovering `.editorconfig`/`.gitattributes` files
  on disk and feeding them into these modules from a real open
  buffer/project — both modules take a path or file content directly and
  leave "which files apply to this editing context" to a future caller,
  the same design language `layering` and `keymap` already established in
  this phase; anything in `lux-core` or the `lux` binary actually acting on
  a resolved `EditorConfig` or `EolNormalization` (auto-indent-detection
  override, EOL conversion on save); a `.gitattributes`-driven merge with
  the layered settings model from the `layering` loop.
