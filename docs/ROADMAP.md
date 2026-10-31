# Roadmap — gap matrix

Updated 2026-10-31 against the desktop binary on this branch (workspace **1.2.0**).
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
| Themes | yes | yes | yes | in shell (Lux Dark plus scheme presets) | shipped |
| Huge files without hitching | yes | weak | yes | read-only above 10 MB. Release mode asserts 8 ms on `insert_at_cursor` plus syntax sync. That is not keypress-to-pixel | P1 |
| Download and run on Linux, macOS, Windows | yes | yes | yes | v1.2.0 tag builds raw binaries plus `lux-linux-amd64.deb`, `lux-macos-aarch64.dmg`, and `lux-windows-x86_64.msi`. The repo is private; a GitHub session is required | this release |
| Installer (dmg notarized, msi, deb/rpm) | yes | yes | yes | unsigned `.deb` and `.msi`, ad-hoc `.dmg` (not notarized). No certificate in CI. rpm/AppImage/Flatpak and winget remain stubs | P1 |
| GUI pass on Linux and Windows | yes | yes | yes | CI drives open, type, save, palette, split, and find through `handle_keyboard_event`. Pixels, IME, and native file dialogs stay manual | **next** |
| Screen reader (VoiceOver, NVDA, Orca) | partial | yes | partial | CI checks the AccessKit tree (roles and buffer/find text). VoiceOver was an earlier macOS pass. NVDA and Orca were not run | **next** |
| Intel Mac binary | yes | yes | yes | not in the release matrix (`macos-latest` is Apple Silicon) | P1 |
| Linux aarch64 binary | yes | yes | yes | not in the release matrix | P2 |
| Auto-update server | yes | yes | yes | Ed25519 verify only. No hosted channel (no paid service) | P2 |
| Debugger, collaboration, marketplace, AI, accounts, telemetry | varies | yes | varies | excluded by [NON-GOALS.md](NON-GOALS.md) | excluded |
| iOS / Android editing on device | no | no | no | hosts exist. Device IME and store builds are not done | P2 |

## Next three (following tanda)

1. **Signed packages and the missing architectures.** v1.2.0 ships an
   ad-hoc macOS `.dmg`, a WiX `.msi`, and a `.deb`. Notarization and
   Authenticode need a certificate that is not in the repo and must not
   be added as a secret from this workflow. Intel Mac and Linux aarch64
   binaries are still absent (`macos-latest` is Apple Silicon;
   `ubuntu-latest` is x86_64).
2. **A real GPU window on Linux and Windows.** The headless session covers
   the editing handlers. It does not cover pixels, the IME candidate
   window, or the native open/save dialog.
3. **VoiceOver, NVDA, and Orca on the AccessKit tree.** CI checks roles
   and the buffer and find-query text. A hardware or OS screen reader was
   not run for this release. Huge-file editing above 10 MB stays P1 beside
   these three.
