# Receipt: G04 — Wire `lux-syntax` highlights into the editor paint path

- Phase: integration / TODO G04
- Crate(s): `lux` (`syntax` module + `app`/`render` wiring), `lux-ui`
  (`VisibleLine::byte_start`), `lux-syntax` (`BackgroundHighlighter::try_recv`)
- Goal: Tree-sitter tokens color glyphs in the running desktop shell for at
  least one language; edits refresh highlights without blocking the UI
  thread; STATUS reflects on-screen reality on verified platforms.
- Verify:
  - `cargo test -p lux-syntax`
  - `cargo test -p lux`
  - `cargo clippy -p lux --all-targets -- -D warnings`
  - `./scripts/check-limits.sh`
  - Deterministic color tests (no GPU):
    `syntax::tests::rust_keyword_fn_gets_lux_dark_keyword_color`,
    `syntax::tests::rust_string_gets_lux_dark_string_color`,
    `syntax::tests::background_worker_delivers_tokens_off_caller_thread`
  - Linux xvfb:
    `timeout 5 env WGPU_BACKEND=gl xvfb-run -a ./target/debug/lux file.rs`
    prints `Lux: syntax highlighting (Rust)` and does not panic
- Iterations: 3 / 5
- Result: passed on Linux (unit color mapping + worker + xvfb open).
  Windows/macOS on-screen shell not run in this Cloud Agent.
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:
  - `lux` depends on `lux-syntax`. `SyntaxHighlight` owns a
    `BackgroundHighlighter`, submits `Job::SetText` on open/edit, and
    drains via `try_recv` in `about_to_wait`.
  - Paint maps each cluster's line-local bytes through
    `VisibleLine::byte_start` to absolute offsets, then
    `SyntaxTheme::color_for_token` (`LUX_DARK`). Untokenized spans keep the
    chrome foreground — no fake colors.
  - Unknown extensions → no highlighter (flat paint). No folding UI (G07).
  - `BackgroundHighlighter::try_recv` added so the UI never blocks on
    `recv` while still using the existing notify/channel worker (no FS or
    highlight polling).
