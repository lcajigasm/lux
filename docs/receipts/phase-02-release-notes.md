# Phase 2 release notes — Rendering & desktop shell

Commits from `phase-1-core-complete` to `HEAD` (14 commits).

## Highlights

- **`lux-platform`**: real `winit`-backed desktop trait implementation —
  window creation, keyboard/mouse/touch input, IME event plumbing, clipboard,
  native file dialogs, DPI/scale-factor, lifecycle, safe-area insets.
- **`lux-render`**: `wgpu` context/surface/resize; a glyph atlas (`etagere` +
  `swash`, 4×1 subpixel) with LRU eviction; real quad/text render pipelines
  with `TextRenderingMode` (platform-default/subpixel/grayscale); `parley` +
  `fontique` text layout with per-run OpenType feature control; a headless,
  event-driven repaint scheduler (no polling).
- **`lux-ui`**: a virtualized editor view (only visible lines shaped),
  multi-cursor caret blink, selection/indent-guide/column-ruler geometry, the
  Lux Dark theme, and a real `accesskit` tree builder.
- **`lux-platform` (accessibility + wakeup)**: per-window `accesskit_winit`
  adapter wiring kept internal to the desktop backend (ADR 0013), and a real
  `request_wakeup(Instant)` mechanism so timer-driven repaints (cursor blink)
  fire without polling.
- **`crates/lux`**: the first unified desktop binary — assembles all of the
  above into a real window with live, readable text (a real macOS system
  monospace font read off disk), real keyboard/mouse editing over a real
  `lux-core::Buffer`, inline IME preedit at the actual cursor position, a
  real `accesskit` tree confirmed with VoiceOver, and the Lux Dark theme
  painted on screen.

## Bug found and fixed during manual verification

`swash::FontRef::from_index` was called fresh on every frame in
`crates/lux/src/render.rs`. `swash::cache::CacheKey::new()` is a global,
per-call-incrementing counter, not derived from font content, so every
frame minted a new, never-matching key — the glyph atlas cache never hit,
and every visible glyph was re-rasterized and evicted every frame. The
project owner reported this directly ("los carácteres que escribo no son
los que se ven"). Fixed by building the `FontRef` exactly once, over font
bytes leaked to `'static`, and reusing it every frame.

## Known gaps (honest, documented, not silently deferred)

- Windows and Linux are `not-started` for the whole phase — no `rustup`
  toolchain available on this machine, so not even a cross-compile check is
  possible for `lux-platform`'s desktop backend.
- NVDA and Orca are unverified (no Windows/Linux environment to run them).
- Gutter line numbers reserve width but aren't painted as glyphs yet.
- `AccessibilityActionRequested` is received but not wired to buffer
  mutation — screen readers can read content/cursor/selection, not drive
  them.
- A lost `wgpu` surface (`CurrentSurfaceTexture::Lost`) isn't recovered.
- The background color feeding the subpixel-compositing shader is
  sRGB-byte-normalized rather than rigorously gamma-correct (internally
  consistent, a polish item, not a correctness bug).
- Combining-mark clusters (more than one real glyph per cluster) only
  expose their first glyph.

## Commits

```
5b07f90 feat(lux-platform): wire accesskit adapter and add render-loop wakeup
9bf0cbb feat: add glyph id to cluster geometry and expose EditorView shaping accessors
c106b98 feat(lux): add unified desktop binary assembling platform, render, and ui
cbcfc43 docs: close out Phase 2 STATUS and receipt for unified lux binary
b792336 docs: close out Phase 2 receipts and STATUS
faed3f4 Add wgpu quad/text pipeline, glyph cache eviction, TextRenderingMode, render loop scheduler
83c210b Add Phase 2 performance budget benchmarks
4f7e232 Add Lux Dark theme
e234fea Add lux-ui accesskit tree builder
a79369d Add lux-platform IME composition tracking
6c5b474 Add lux-ui virtualized editor view
38eed33 Add parley + fontique text layout integration
3018a08 Add glyph atlas (etagere + swash, 4x1 subpixel)
5cc21a9 Add lux-platform winit backend and lux-render wgpu foundation
```
