# macOS parity

Lux's product focus is macOS. Linux and Windows stay available and are
secondary. This table compares the desktop shell with Sublime Text, VS Code,
and Zed. "«pendiente de verificar en Mac»" means the code is in the tree but
this environment is Linux, so a real Mac has not clicked, scrolled, or
launched the disk image.

| Capability | Sublime | VS Code | Zed | Lux | Verification |
|---|---|---|---|---|---|
| Standard Cmd shortcuts (save, find, palette, goto) | yes | yes | yes | Sublime keymap binds Cmd on macOS (`crates/lux-config/keymaps/sublime.jsonc`) | «pendiente de verificar en Mac» |
| Native menu bar | yes | yes | yes | AppKit menu in `crates/lux/src/menu/macos.rs` | «pendiente de verificar en Mac» |
| Cmd+P / Cmd+Shift+P | Cmd+P / Cmd+Shift+P | same | Cmd+P / Cmd+Shift+P | Goto Anything and the command palette | Linux headless test drives the palette. Menu click: «pendiente de verificar en Mac» |
| Multi-cursor | yes | yes | yes | In the shell (Cmd+D, column select) | «pendiente de verificar en Mac» |
| Minimap | yes | yes, optional | optional | In the shell (`crates/lux/src/minimap.rs`), toggle from the view menu | Draw on a Retina window: «pendiente de verificar en Mac» |
| Light / dark follows the system | yes | `window.autoDetectColorScheme` | yes | `workbench.colorTheme` default `system` maps a platform scheme to Lux Dark or Lux Light. A named theme wins | Mapping is unit-tested. Live switch: «pendiente de verificar en Mac» |
| SF Mono, then Menlo | SF Mono / Menlo | editor font, often Menlo | similar | Candidate list tries SF Mono paths, then Menlo, then Monaco | Order is unit-tested. Which file macOS opens: «pendiente de verificar en Mac» |
| Retina / HiDPI | yes | yes | yes | `NSHighResolutionCapable` and `scale_factor` on layout | Pixel quality: «pendiente de verificar en Mac» |
| Inertial trackpad scroll | yes | yes | yes | Precise wheel events are physical pixels on both axes. macOS momentum is more of those events | Math is unit-tested. Feel: «pendiente de verificar en Mac» |
| Tabs | yes | yes | yes | Tab strip in the shell | «pendiente de verificar en Mac» |
| Open from Finder / `lux .` | yes | yes | yes | `lux .` opens the folder. `Info.plist` lists `public.folder` | `lux .` classification is unit-tested. Finder handoff: «pendiente de verificar en Mac» |
| Native window, traffic lights | yes | yes | yes | winit decorated window (close, minimize, zoom). No custom titlebar | «pendiente de verificar en Mac» |

## What changed in this pass

These are the gaps a Linux VM can implement and test:

1. **SF Mono before Menlo.** The loader looks at `SFNSMono.ttf`,
   `Supplemental/SF-Mono-Regular.otf`, and `/Library/Fonts/SF-Mono-Regular.otf`
   before Menlo.
2. **System appearance.** `PlatformEvent::ColorSchemeChanged` and the
   window's current scheme select Lux Light or Lux Dark while the setting
   is `system` or `auto`. The new default setting is `system`. An explicit
   theme (including the theme picker) stops following the OS.
3. **Editor clear color.** Text subpixel compositing uses
   `chrome.theme.background` instead of a hardcoded Lux Dark color, including
   after a theme change and on resize.
4. **Trackpad scroll.** Pixel deltas move `scroll_x` and `scroll_y` in
   physical pixels (winit already converts macOS points, including the
   momentum phase). Line deltas still scale by `LINE_HEIGHT * scale_factor`.
5. **`lux .`.** A directory argument opens the workspace. The folder test
   checks the sidebar.

## Still needs a Mac

Ad-hoc `codesign`, `hdiutil`, the universal `lipo` binary, Gatekeeper
quarantine (`xattr`), traffic lights, the native menu, Retina drawing,
trackpad inertia as a feeling, and Finder "Open With". The disk image is
not notarized. There is no Apple Developer ID in this repo.

`scripts/package-macos.sh` builds that image when both
`aarch64-apple-darwin` and `x86_64-apple-darwin` standard libraries are
installed. `.github/workflows/release-macos.yml` runs that script on
`macos-14` when a GitHub Release is published, or on `workflow_dispatch`.
It does not run on pull request, branch push, or tag push.
