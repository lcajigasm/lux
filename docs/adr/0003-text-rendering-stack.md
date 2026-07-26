# 0003. Text rendering stack: `parley` + `harfrust` + `fontique` + a custom atlas on `swash`; not `vello`

## Status

Accepted.

## Context

Lux needs production-quality text layout and rasterization across five
platforms, with subpixel antialiasing competitive with native apps.

- **`parley`** 0.11.0 is the strongest Rust text layout engine available: CSS
  line-box semantics, bidi, CJK/Thai line breaking, cluster geometry (needed
  for correct IME and selection), optional `accesskit` integration. Since
  0.6.0 it shapes via `harfrust` instead of `swash`, and its own changelog
  calls it production-quality for all scripts.
- **`harfrust`** 0.12.0 lives in the HarfBuzz org, co-authored by Behdad
  Esfahbod, tracks HarfBuzz 13, is safe Rust, and runs within ~25% of
  HarfBuzz's speed. `rustybuzz` is frozen (no 2026 commits); `harfbuzz-rs` has
  been dead since 2021.
- **`swash`** 0.2.10 is the only crate offering `Format::subpixel_bgra()`
  with hinting — required for text that doesn't look like a toy next to
  native rendering.
- **`vello`** is explicitly rejected for editor text, and the reason is
  architectural, not a release-calendar problem: its README still says
  alpha, it has no glyph atlas (issue #204, open, blocking), and it
  path-renders every glyph every frame — untenable at tens of thousands of
  glyphs per frame. It also has no subpixel LCD antialiasing on any path,
  and its own roadmap acknowledges the underlying problem: you cannot render
  RGB subpixel output onto a transparent RGBA layer. Neither `vello` nor
  `parley` has a 1.0.
- Re-verified 2026-07-26: issue #204 is still open. The `glifo` 0.1.1 crate,
  cited in the original research as an independently-converged reference for
  the same 4×1 subpixel-bucket atlas design, turned out to have moved into
  the `vello` repository itself, specifically to iterate on `vello`'s own
  glyph-atlas gap — it is a `vello`-internal effort, not outside validation
  that the gap is closing on its own. This doesn't change the decision, but
  it changes how much weight `glifo` carries as evidence.
- **`fontique`**, not `fontdb`, for system font enumeration: `fontdb` hasn't
  published since October 2024, doesn't talk to the system font API, and
  depends on `ttf-parser`, which carries an unmaintained RUSTSEC advisory.

## Decision

Layout and shaping: `parley` 0.11.0 + `fontique` 0.11.0 (which brings in
`harfrust` 0.12.0). Rasterization: `swash` 0.2.10, feeding a custom glyph
atlas built with `etagere` (0.3.0). Atlas design: 4 horizontal subpixel
variants × 1 vertical, separate textures per glyph kind (`Monochrome`,
`Polychrome`, `Subpixel`) — the same layout `gpui` already validated in
production. `lux-render` owns the atlas; nothing above it touches `swash` or
`etagere` directly.

## Alternatives considered

`vello` (rejected, see above). `glyphon` (0.12.0, tracks `wgpu` 30) remains
the documented fallback if Phase 2's custom atlas can't hit its latency or
quality budget — that decision is reserved for the project owner at the
Phase 2 gate, not made here.

## Consequences

We take on maintaining a glyph atlas and cache-eviction policy ourselves.
In exchange, we avoid `vello`'s per-frame path-rendering cost and its
subpixel-AA gap, and we avoid a hard version conflict: `vello` 0.9 pins to
`wgpu` 29 while `glyphon` 0.12 pins to `wgpu` 30 — the two cannot coexist in
one binary, which is itself an argument for owning this layer rather than
inheriting either chain. `parley` breaks its own API every 6–10 weeks
(0.8→0.11 within 2026 alone) and has an in-flight refactor introducing new
subcrates (`parlance`, `parley_engine`); `lux-render` wraps it behind our own
layout API so that churn stays contained.
