# Receipt: `lux-config`: JSONC, layered precedence, hot reload (narrowed slice of Phase 5 deliverable 1)

- Phase: 05
- Crate(s): lux-config
- Goal: The "lux-config: JSONC, precedencia por capas, recarga en caliente"
  deliverable of Phase 5 ("Configuración, keymaps y personalización") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-config layering::`
- Iterations: 2 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Starting state.** `lux-config` was an empty stub crate (`src/lib.rs` was
  two blank lines), same as every other not-yet-built crate. Nothing in the
  repo referenced it.

  **Scope: a narrower slice than the full master-prompt bullet, by explicit
  instruction.** Phase 5's deliverable 1 text is "configuración en JSONC con
  comentarios, con precedencia clara y documentada por defecto → usuario →
  proyecto → carpeta → lenguaje, recarga en caliente, esquema publicado y
  validación con errores útiles señalando línea y columna." This loop's
  goal named only "JSONC, precedencia por capas, recarga en caliente" —
  JSONC parsing, the five-layer precedence order, and hot reload. Schema
  publication and schema-validation-with-line/column are explicitly not in
  scope here (a separate, later loop); the JSONC parser chosen does surface
  line/column on a parse failure as a side effect of the dependency itself,
  but no schema or validator was built. `docs/STATUS.md`'s "JSONC config
  with layered precedence" row stays `not-started` (rule 1), with a scope
  note — same pattern as every Phase 4 deliverable: this is a headless
  library with nothing wired into the `lux` binary yet, no keymaps, no
  `.editorconfig`, no preferences surface.

  **Dependency choice: `jsonc-parser` 0.33.1, not a hand-rolled comment
  stripper.** `docs/DEPENDENCIES.md` had this earmarked as "no fixed
  version yet; picked when Phase 5 lands" since Phase 0. Confirmed the real
  resolution via `cargo add -p lux-config jsonc-parser --features serde`
  (0.33.1) rather than guessing a version, and read the actual crate source
  (`~/.cargo/registry/.../jsonc-parser-0.33.1/src/serde.rs`) to find
  `parse_to_serde_value::<serde_json::Value>`, which deserializes JSONC
  straight into `serde_json::Value` with no intermediate AST conversion
  step. `ParseOptions::default()` allows comments, trailing commas,
  single-quoted strings, loose object keys, and hex/unary-plus numbers — a
  real superset of strict JSONC (VS Code's comments-plus-trailing-commas),
  not a narrower hand-picked subset; a real, already-tested parser doing
  real work, not a regex-based comment stripper reinventing part of a JSON
  grammar. MIT-licensed, already in `deny.toml`'s allow list — no new entry
  needed. `notify` 8.2.0 — the same pin `lux-workspace::watcher` already
  uses (Phase 4) — was added for hot reload; already CC0-1.0-allowed in
  `deny.toml` from that earlier entry. Both recorded in
  `docs/DEPENDENCIES.md`.

  **Layering model: five layers, recursive JSON-object merge, arrays
  replaced wholesale.** `Layer` is a variant-only enum
  (`Default < User < Project < Folder < Language`, `derive(Ord)` ordering
  by declaration position) so `Layer::ALL` iterated in order and merged
  left-to-right gives later layers priority automatically — the precedence
  order is enforced by the type, not by a separately-maintained list that
  could drift from it. `merge()` recurses into nested objects present on
  both sides and lets a higher layer's scalar or array fully replace
  whatever a lower layer had at that key otherwise; concatenating or
  splicing arrays across layers was considered and dropped — no settings
  system with this kind of layering (VS Code included) merges array
  *contents* across layers, because there's no general, unsurprising
  definition of what that would mean for e.g. an ordered exclude-glob list.
  `Layer::Default` is never file-backed (it's the built-in defaults
  document, parsed once at construction) and is rejected with an
  assertion by every method that takes a `Layer` and touches a file
  (`set_layer_path`, `clear_layer`, `reload_layer`) — a caller trying to
  point the default layer at a file is a programming error, not a runtime
  condition to recover from.

  **"A malformed configuration never leaves the editor unusable or loses
  settings" (this phase's acceptance criterion) is a real behavioral
  guarantee, not just a docstring.** `reload_layer` only overwrites a
  layer's cached value on a *successful* parse; a parse failure returns
  `Err(LayerError::Parse(_))` but leaves the layer's previous value (if
  any) untouched, so `resolved()` keeps returning the last good merge.
  Exercised directly
  (`malformed_layer_keeps_the_previous_good_value_and_reports_the_error`):
  write a valid layer, reload, corrupt the file, reload again, assert the
  merge is unchanged even though the reload call returned an error. A
  *missing* file is deliberately not the same failure mode — it's treated
  as "this layer was never set," clearing its contribution rather than
  erroring, since settings files are optional by design
  (`missing_layer_file_contributes_nothing_without_erroring`,
  `reload_after_the_file_is_deleted_falls_back_silently`). The one failure
  this can't degrade gracefully from is the *default* document itself
  failing to parse (`LayeredConfig::new`) — there's no previous good value
  to fall back to for a layer that's never successfully loaded once, so
  that's a hard `Err` from construction rather than folded into the
  keep-the-last-good-value handling.

  **Hot reload: `notify` directly, no debouncer, watching each layer's
  *parent directory* rather than the file itself.** Unlike
  `lux-workspace::watcher` (which debounces bursts of events across a
  whole project tree and exists specifically to normalize that), settings
  files change one at a time and rarely, so `ConfigWatcher` reacts to every
  raw `notify` event directly. It watches each file-backed layer's parent
  directory, not the file: `notify::Watcher::watch` requires the target to
  exist, but a settings file legitimately might not yet (nothing requires
  `~/.config/lux/settings.jsonc` to already exist) — watching the
  directory lets a file created later start contributing without
  `ConfigWatcher` needing to be torn down and rebuilt
  (`creating_a_previously_missing_file_hot_reloads`). The directory itself
  is still assumed to exist; creating config directories is left to a
  future caller/init step, not this module's job. `ConfigWatcher::new`
  loads every layer synchronously before returning (via
  `LayeredConfig::set_layer_path`, which both registers the path and does
  the first load), so a caller gets an already-current config back instead
  of racing the first filesystem event.

  **A real bug, not a flake, caught by the first test run.** The first
  `cargo test -p lux-config layering::` pass failed
  `creating_a_previously_missing_file_hot_reloads` (expected value
  `"created"`, got `"default"` — the reload never actually happened) and
  then aborted the whole process (`SIGABRT`) from a second panic on the
  `notify-rs fsevents loop` thread trying to `.lock().unwrap()` a
  now-poisoned mutex — a poisoned lock reached from a non-unwinding FFI
  callback context aborts the process instead of just failing one test.
  Root cause, found by reading `ConfigWatcher::new` rather than guessing at
  FSEvents timing: the initial-load loop was calling `reload_layer`
  directly instead of `set_layer_path`, so a layer `LayeredConfig` had
  never been told about via `set_layer_path` stayed absent from its
  internal map — `reload_layer` is a no-op for a layer it's never heard
  of, so the file was never actually read even after the real filesystem
  event fired and the callback dutifully reported `Reloaded` (the reload
  attempt "succeeded" by doing nothing, since a never-registered layer's
  `reload_layer` short-circuits to `Ok(())`). Fixed by having the initial
  loop call `set_layer_path` (which both registers the path and performs
  the first load) instead. Re-ran the full `layering::` suite four times
  consecutively after the fix, all green each time, to rule out remaining
  flakiness from the real FSEvents timing involved — same precaution
  `lux-workspace`'s watcher receipt took for the same class of risk.

  **Actual verification.** `cargo test -p lux-config layering::`: 19/19
  passed, 0 ignored — 1 real failure (the bug above) on iteration 1, fixed
  on iteration 2, green on that run and three subsequent consecutive
  re-runs. Coverage: JSONC parsing (comments, trailing commas, nested
  objects/arrays, empty-input-as-null, a malformed-input error whose
  message carries line and column — 4 tests in `layering::jsonc::tests`);
  merge semantics (default-only resolution, higher-layer override at the
  same key, recursive merge preserving untouched sibling keys, arrays
  replaced wholesale rather than spliced, the full five-layer precedence
  chain peeled off one layer at a time to prove the order rather than just
  that the topmost layer wins, malformed-layer resilience, missing-file
  and deleted-file fallback, `clear_layer`, and the default document's own
  parse failure as a hard error — 10 tests in `layering::tests`); hot
  reload against a real filesystem and a real `notify` backend (starts
  already reflecting current file contents, editing a watched file
  reloads without restart, creating a previously-missing file reloads once
  it appears, a malformed edit reports `Failed` while the merge keeps its
  last good value, and two layers in different directories reload
  independently of each other — 5 tests in `layering::watch::tests`). Full
  repo gate also run and green: `cargo fmt --all --check` (after one
  `cargo fmt --all` pass to reformat a few over-width lines in the new
  test code); `cargo clippy --workspace --all-targets -- -D warnings`
  (zero warnings, including on the two new dependencies); `cargo test
  --workspace` (all crates, no regressions — `lux-core`'s 255,
  `lux-workspace`'s 54, `lux-config`'s new 19, everything else unchanged);
  `cargo deny check` (advisories/bans/licenses/sources all `ok`; the only
  output is a pre-existing `windows_x86_64_msvc` duplicate-version
  *warning*, unrelated to this change and already below the
  `multiple-versions = "warn"` threshold in `deny.toml`, not a failure);
  `scripts/check-limits.sh` (`layering/mod.rs` at 429 lines and
  `layering/watch.rs` at 289, both under the 600-line cap; no struct
  exceeds 4 fields, well under the 25-field limit);
  `scripts/check-core-isolation.sh` (unaffected — it only checks
  `lux-core`, and `lux-config` isn't a dependency of it). Verified on
  macOS (aarch64-apple-darwin) only — same documented limitation as the
  rest of `docs/STATUS.md` (no non-macOS dev environment available yet);
  the hot-reload tests in particular exercise the real FSEvents backend
  specifically, so Linux's inotify and Windows' `ReadDirectoryChangesW`
  paths through `notify` are genuinely unverified, not just assumed fine.

  **What's out of scope and untouched:** schema publication and
  schema-validation-with-line/column (the rest of this phase's deliverable
  1 — `jsonc-parser`'s own errors already carry line/column for a raw
  parse failure, which is as far as this loop goes); keymaps, Sublime/VS
  Code/JetBrains presets, `.editorconfig`/`.gitattributes`, settings
  importers, typography settings, and a preferences surface (Phase 5
  deliverables 2-6); resolving *which* file backs the
  `Project`/`Folder`/`Language` layers for a given editing context (which
  project, which workspace folder, which language id) — `LayeredConfig`
  and `ConfigWatcher` merge whatever paths they're given, in the fixed
  precedence order, but choosing those paths from an open workspace/buffer
  is a future integration's job; wiring any of this into the `lux` binary
  or any UI; creating missing config directories.
