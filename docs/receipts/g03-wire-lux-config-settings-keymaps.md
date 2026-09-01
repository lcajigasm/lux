# Receipt: G03 — Wire `lux-config` into `lux` (settings + keymaps)

- Phase: integration / TODO G03
- Crate(s): `lux` (`config` module + `app`/`render` wiring), `lux-ui`
  (`EditorView::{metrics,set_metrics}`), `lux-config` (existing layering /
  keymap / font_settings)
- Goal: Layered JSONC settings and the Sublime keymap preset actually drive
  input and editor typography in the running binary; hot reload via
  `ConfigWatcher` (notify) with no polling; no settings UI for unimplemented
  controls.
- Verify:
  - `cargo test -p lux-config`
  - `cargo test -p lux`
  - `cargo clippy -p lux --all-targets -- -D warnings`
  - `./scripts/check-limits.sh`
  - Linux xvfb smoke:
    `timeout 5 env WGPU_BACKEND=gl xvfb-run -a ./target/debug/lux`
    prints `Lux: config watching … (keymap: Sublime Text)` and does not
    panic
  - Hot-reload covered by
    `config::tests::watched_user_settings_hot_reload_updates_font_size`
    (same `ConfigWatcher` + `font_settings::resolve` path `about_to_wait`
    drains)
  - Keymap dispatch covered by
    `config::tests::sublime_save_and_lux_open_resolve_on_linux`
- Iterations: 3 / 5
- Result: passed on Linux (unit keymap + font metrics + notify reload +
  xvfb boot smoke). Windows / macOS first-frame not run in this Cloud
  Agent — STATUS for those platforms stays `not-started`.
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:
  - `crates/lux/src/config.rs` bootstraps `LayeredConfig` defaults, watches
    `$XDG_CONFIG_HOME/lux/settings.jsonc` (or platform equivalent), builds a
    single-chord reverse index from Sublime + Lux `open`/`save_as` overlays,
    and maps only wired commands (`save` / `open` / `save_as`).
  - Unwired preset chords (find, undo, …) are not indexed — they fall
    through to normal typing rather than simulating missing features.
  - `about_to_wait` drains `ConfigWatcher::try_recv` and applies
    `EditorView::set_metrics`; blink wakeups already keep the loop alive so
    this is not a poll of the filesystem.
  - Paint path (`render.rs`) reads live `editor_view.metrics()` instead of
    the old `FONT_SIZE` / `LINE_HEIGHT` constants.
  - No preferences GUI (rule #1). Importers / editorconfig / schema
    completion remain library-only.
  - Out of scope: multi-chord sequences, theme wiring (G04+), preferences
    surface.
