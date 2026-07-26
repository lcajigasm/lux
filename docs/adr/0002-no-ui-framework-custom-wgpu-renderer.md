# 0002. No Rust UI framework; a custom renderer on `wgpu`

## Status

Accepted.

## Context

Lux needs custom GPU-drawn text rendering (subpixel antialiasing, a glyph
atlas, a code-editor-shaped rendering model) on five platforms: macOS,
Windows, Linux, iPadOS/iOS, Android. As of the 2026-07-25/26 research window,
no Rust UI framework covers both requirements at once:

- **`gpui`** (Zed): the published crate (0.2.2, October 2025) is
  architecturally stale and its own install snippet doesn't work, because
  `gpui_platform` isn't published. Real usage is an unversioned `git` pin
  against Zed's repo with weekly breaking changes. The Zed team has stated
  they don't have the resources to maintain it as an external library. No
  mobile support (tracking issues open since November 2025, "Wide Scope,"
  unassigned). Also a licensing mismatch: `gpui` itself is Apache-2.0, but
  Zed's editor crates are GPL-3.0-or-later, which is not something Lux (MIT
  OR Apache-2.0) can build on without care.
- **`floem`** (Lapce) and **`iced`**: desktop-only.
- **`slint`**: the only one with credible iOS and Android support, but the
  worst fit for custom text rendering — its own DSL, nascent rich text, and
  custom painting only through an `unstable-wgpu-29` escape hatch.
- **`freya`**: moving toward mobile, but Android is experimental and there
  is no iOS.
- **`xilem`/`masonry`**: self-described alpha, no iOS.

## Decision

Build directly on `wgpu` 30.0.0, with no UI framework layer. `lux-render`
owns the GPU context, resize/surface handling, and drawing primitives.
`lux-ui` owns widgets, layout, and interaction on top of that. This is the
only path that keeps iOS reachable on our own terms, and it mirrors what
`gpui` itself does under its hood — we're choosing to own that layer instead
of inheriting an unmaintainable pin to someone else's.

Re-verified 2026-07-26: `gpui` unchanged (still 0.2.2, no release since). No
reason to revisit.

## Alternatives considered

Each framework above was considered and rejected for the reason listed. The
fallback discussed in the master prompt if Phase 2's custom renderer misses
its latency/quality budget — native per-platform text backends (CoreText,
DirectWrite, `cosmic-text`) behind the same `lux-platform` trait, or
`cosmic-text` + `glyphon` as a faster path to pixels — is explicitly a
decision for the project owner to make at that gate, not a default to fall
back to silently.

## Consequences

`wgpu` breaks its API roughly every 12 weeks by stated policy. All `wgpu`
usage is isolated inside `lux-render`; a major-version bump is a scoped,
planned PR, not a workspace-wide scramble. We accept the cost of writing our
own glyph atlas, quad pipeline, and text pipeline in exchange for not being
blocked on someone else's mobile roadmap.
