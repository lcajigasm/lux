# Architecture

High-level shape of the workspace (full crate-by-crate rationale lives in
`docs/adr/`; this page is the map, not the reasoning).

## Layering

```
lux-core        headless text engine — zero UI dependencies (golden rule #2)
lux-syntax      tree-sitter highlighting, token themes        \
lux-search      find-in-files, fuzzy matching                  \  depend only
lux-config      JSONC settings, keymaps, .editorconfig          > on lux-core
lux-workspace   projects, FS watchers, sessions                /  and stdlib-
lux-git         gix-backed Git integration                    /   ish crates
lux-lsp         LSP client                                    /
lux-term        PTY + VT terminal (desktop only)              /
lux-plugin      wasmtime WASM plugin host                    /

lux-platform    platform trait (window/input/IME/clipboard) + winit desktop impl
lux-render      wgpu context, glyph atlas, drawing primitives
lux-ui          widgets, layout, interaction — the only crate allowed to know
                about both the domain crates above and lux-render/lux-platform

lux             desktop binary: wires lux-ui to a lux-platform backend
```

`cargo tree -p lux-core` must never show `wgpu`, `winit`, `parley`, `swash`,
or `accesskit` — enforced by `scripts/check-core-isolation.sh` and CI. The
same independence is why `lux-core` builds for `aarch64-apple-ios` and
`aarch64-linux-android` from Phase 1 onward: it's a portability check, not a
feature.

## Why a platform trait instead of `winit` everywhere

`winit` cannot be embedded inside a `UIView` or `SurfaceView`, and its mobile
IME support is shows/hide-only — both documented, verified limitations (see
`docs/adr/0004-platform-trait-and-mobile-backends.md`). `lux-platform` defines
the trait once; `winit` backs it on desktop (Phase 2), and hand-written
UIKit/JNI implementations back it on iOS (Phase 12) and Android (Phase 13).
UI code above the trait never branches on platform.

## Why no UI framework

No Rust UI framework in mid-2026 covers both "custom GPU code-editor
rendering" and all five target platforms. See
`docs/adr/0002-no-ui-framework-custom-wgpu-renderer.md` for the crate-by-crate
elimination (`gpui`, `floem`, `iced`, `slint`, `freya`, `xilem`/`masonry`).

## Capability profile per platform

Desktop (macOS/Windows/Linux) ships the full editor, terminal, LSP, and Git.
Mobile (iPadOS/iOS) ships the editor, Git, and syntax — and explicitly no
terminal UI and no LSP UI, because iOS sandboxed apps cannot spawn child
processes (`fork`/`exec`/`posix_spawn` are unavailable). This is not a
missing feature to fill in later; it's a platform constraint reflected as
data in the capability profile and in `docs/STATUS.md`, per golden rule #1 —
see `docs/adr/0006-mobile-capability-profile.md`.

## Where decisions live

Every choice above has a corresponding ADR in `docs/adr/`. If you're about to
revisit one, read the ADR first — reopening it without new evidence wastes a
cycle re-deriving what's already written down.
