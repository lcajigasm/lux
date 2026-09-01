# Receipt: G05 — Tabs + status bar wired and painted

- Phase: integration / TODO G05
- Crate(s): `lux` (`tab_session`, `tabs_actions`, `chrome_paint`), `lux-ui`
  (`tabs::TabBar::set_title`)
- Goal: Multi-buffer tabs and a status bar are visible and operable in the
  desktop shell using existing `lux-ui` modules; STATUS reflects on-screen
  reality on verified platforms.
- Verify:
  - `cargo test -p lux-ui -- tabs:: status_bar::`
  - `cargo test -p lux`
  - `cargo clippy -p lux --all-targets -- -D warnings`
  - `./scripts/check-limits.sh`
  - Deterministic session tests (no GPU):
    `tab_session::tests::open_second_file_adds_a_tab`,
    `tab_session::tests::focus_existing_switches_without_duplicating`,
    `tab_session::tests::close_active_loads_neighbor`,
    `tab_session::tests::hit_test_distinguishes_activate_and_close`
  - Linux xvfb: opening a file prints `Lux: tabs + status bar` and does not
    panic; opening a second file via Open prints `Lux: tabs open=2`
- Iterations: 4 / 5
- Result: passed on Linux (unit tab session + xvfb open). Windows/macOS
  on-screen shell not run in this Cloud Agent.
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:
  - `TabSession` parks inactive documents; active bytes stay in `App` fields.
  - Paint reserves `TAB_STRIP_HEIGHT` / `STATUS_BAR_HEIGHT`; status labels come
    from `status_bar::build` (cursor/language/encoding/EOL/indent).
  - Click tab to activate; click × or middle-click / Ctrl+W to close;
    Ctrl+Tab cycles; Ctrl+Shift+T reopens. DnD polish out of scope.
  - `TabBar::set_title` added so dirty markers can update the strip.
