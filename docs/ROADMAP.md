# Roadmap — gap matrix

Updated 2026-10-31 against the desktop binary on `main` (workspace **1.1.1**).
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
those failures. It does not add a new editor surface.

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
| Huge files without hitching | yes | weak | yes | read-only above 10 MB. Release-mode 8 ms typing budget is not what debug CI measures | P1 |
| Download and run on Linux, macOS, Windows | yes | yes | yes | v1.1.0 is macOS-only. v1.1.1 tag builds unsigned `lux-linux-x86_64`, `lux-macos-aarch64`, `lux-windows-x86_64.exe` | this release |
| Installer (dmg notarized, msi, deb/rpm) | yes | yes | yes | `scripts/package.sh` and a winget stub. No signed installer in CI | **next** |
| GUI pass on Linux and Windows | yes | yes | yes | automated tests run on those runners. No recorded human window pass | **next** |
| Screen reader (VoiceOver, NVDA, Orca) | partial | yes | partial | AccessKit tree is built. No hardware pass is recorded | **next** |
| Intel Mac binary | yes | yes | yes | not in the release matrix (`macos-latest` is Apple Silicon) | P1 |
| Linux aarch64 binary | yes | yes | yes | not in the release matrix | P2 |
| Auto-update server | yes | yes | yes | Ed25519 verify only. No hosted channel (no paid service) | P2 |
| Debugger, collaboration, marketplace, AI, accounts, telemetry | varies | yes | varies | excluded by [NON-GOALS.md](NON-GOALS.md) | excluded |
| iOS / Android editing on device | no | no | no | hosts exist. Device IME and store builds are not done | P2 |

## Next three (following tanda)

1. **Installable packages.** The tag workflow ships raw binaries. A download
   should be a double-click: signed macOS `.dmg`, Windows `.msi` or a
   working winget manifest, and a `.deb` that depends on `libfontconfig`
   and the GPU loader. Unsigned raw ELF/EXE is the step before that, not
   the substitute.
2. **Linux and Windows window pass.** Exercise open, type, save, command
   palette, split, and find on a real window on each OS. CI covers the
   handlers. It does not cover presentation, IME candidate windows, or
   file dialogs.
3. **Release-mode input budget and a screen-reader pass.** Re-measure the
   8 ms typing budget with `cargo test --release` on Windows (debug CI
   allows 80 ms). Run VoiceOver or NVDA against the AccessKit tree and
   write the result into `docs/STATUS.md`.
