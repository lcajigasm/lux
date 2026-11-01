# Roadmap — gap matrix

Updated 2026-11-01 against the desktop binary on this branch (workspace **1.2.0**).
macOS is the product focus. Linux and Windows remain secondary.
This page is the prioritized gap list. [GOALS.md](../GOALS.md) is the original
phase plan. [NON-GOALS.md](NON-GOALS.md) stays binding: debugger, live
collaboration, a hosted extension marketplace, accounts, telemetry, and AI
chat are out of scope through v1.0 and are not gaps to implement here.
[AUDIT.md](AUDIT.md) is the 2026-09-08 inventory; later commits wired Git,
LSP, the terminal, and the command palette into the shell.

Comparison targets: Sublime Text 3 (the quality bar), VS Code (breadth we
are not chasing), Zed (native GPU editor with a similar performance claim).

Legend: **in shell** means a user can reach it from the Lux desktop binary.
**crate only** means the domain crate exists and the shell does not expose it.

## What the shell already does

Editing (transactional undo, multicursor, snippets), command palette, Goto
Anything, in-buffer find/replace, project find-in-files, file tree, tabs,
splits, minimap, status bar, themes, keymaps, Git gutter/hunks, an LSP
client, a PTY terminal, WASM plugins, and English/Spanish strings are wired
in `crates/lux` and covered by that crate's tests. CI on 2026-10-29 was red
on every OS (`cargo test` and `cargo clippy -D warnings`). v1.1.1 fixes
those failures. v1.2.0 adds unsigned installers and a headless desktop
session in CI. It does not add a new editor surface.

## Matrix

| Capability | Sublime Text 3 | VS Code | Zed | Lux desktop | Priority |
|---|---|---|---|---|---|
| Open / edit / save, atomic save | yes | yes | yes | in shell. Final newline on save is **off** unless `editor.insertFinalNewline` is set. A rejected save drops that transform | shipped |
| Multicursor, column select, next occurrence | yes | yes | yes | in shell | shipped |
| Command palette | yes | yes | yes | in shell | shipped |
| Goto file / symbol | yes | yes | yes | in shell. `Ctrl+Shift+R` / `Cmd+Shift+R` opens project symbols | shipped |
| Find in buffer and in project | yes | yes | yes | in shell | shipped |
| File tree | yes | yes | yes | in shell | shipped |
| Splits | yes (`Alt+Shift+1..5` on Windows/Linux, `Cmd+Option+1..5` on macOS) | yes | yes | in shell. `Ctrl+Alt+digit` is an alias and does not eat AltGr text | shipped |
| Syntax highlighting | yes | yes | yes | in shell, tree-sitter | shipped |
| Git blame, commit, diff | yes (plugin) | yes | yes | in shell via `gix` | shipped |
| LSP completion, diagnostics, rename | no (plugins) | yes | yes | in shell | shipped |
| Integrated terminal | no in ST3 core | yes | yes | in shell | shipped |
| Themes | yes | yes | yes | in shell. `workbench.colorTheme` defaults to `system` (Lux Dark / Lux Light). A named theme wins | shipped in code; live OS switch is «pendiente de verificar en Mac» |
| Huge files without hitching | yes | weak | yes | read-only above 10 MB. Release mode asserts 8 ms on `insert_at_cursor` plus syntax sync. That is not keypress-to-pixel | P1 |
| Download and run on Linux, macOS, Windows | yes | yes | yes | macOS universal `.dmg` is `release-macos.yml` (`macos-14`, dispatch or `v*` tag only, not started). Linux tag workflow still publishes `lux-linux-x86_64` and `lux-linux-amd64.deb`. Windows `.msi` is outside CI. The repo is private | this release |
| Installer (dmg notarized, msi, deb/rpm) | yes | yes | yes | unsigned `.deb` in the Linux tag workflow. Ad-hoc universal `.dmg` script and opt-in macOS workflow (not notarized, not run). Unsigned `.msi` script. rpm/AppImage/Flatpak and winget remain stubs | P1 |
| GUI pass on Linux and Windows | yes | yes | yes | Linux CI drives open, type, save, palette, split, and find through `handle_keyboard_event`. macOS and Windows are not in CI. Pixels, IME, and native file dialogs stay manual | **next** |
| Screen reader (VoiceOver, NVDA, Orca) | partial | yes | partial | Linux CI checks the AccessKit tree (roles and buffer/find text). VoiceOver was an earlier macOS pass. NVDA and Orca were not run | **next** |
| Intel Mac binary | yes | yes | yes | `scripts/package-macos.sh` lipos arm64 + x86_64 when both stds are installed. The opt-in `macos-14` workflow does that. Not run here | P1 |
| Linux aarch64 binary | yes | yes | yes | not in the release matrix | P2 |
| Auto-update server | yes | yes | yes | Ed25519 verify only. No hosted channel (no paid service) | P2 |
| Debugger, collaboration, marketplace, AI, accounts, telemetry | varies | yes | varies | excluded by [NON-GOALS.md](NON-GOALS.md) | excluded |
| iOS / Android editing on device | no | no | no | hosts exist. Device IME and store builds are not done | P2 |

## Next three

1. **Run the universal `.dmg` once the macOS minute cost is accepted.**
   `release-macos.yml` is capped at 20 real minutes (200 billed minutes at
   the private-repo 10× rate) and has not been dispatched. Notarization
   still needs an Apple Developer ID that is not in the repo. Confirm
   quarantine, traffic lights, and the lipo binary on a Mac
   ([macos-parity.md](macos-parity.md)).
2. **A real GPU window.** The Linux headless session covers the editing
   handlers. It does not cover pixels, the IME candidate window, or the
   native open/save dialog. On macOS that check is «pendiente de verificar en Mac».
3. **VoiceOver on the Mac**, then NVDA and Orca. Linux CI checks AccessKit
   roles and the buffer and find-query text. A hardware screen reader was
   not run. Huge-file editing above 10 MB stays P1 beside these three.
