# Receipt: Accessibility, i18n, chrome wiring (Phase 10)

- Phase: 10
- Crate(s): lux-ui, lux, lux-git
- Goal: Keyboard focus ring, high-contrast WCAG AA theme, reduced motion,
  en/es catalog, RTL direction, OS scale clamp; `lux` paints `Chrome.theme`
  and skips blink when reduced; surfaces call git/term/plugin for real.
- Verify: `cargo test -p lux-ui`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  F6 cycles panels, Escape returns to the editor (no focus trap). VoiceOver/
  NVDA/Orca were not run on this machine — screen-reader rows stay
  `not-started` in STATUS. `cargo test -p lux-ui` 133 + 7 golden tests clean.
  `cargo check -p lux` clean.
