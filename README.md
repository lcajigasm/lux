# Lux

<p align="center">
  <strong>The ultra-fast, zero-bloat native code editor engineered in Rust.</strong><br>
  Sublime Text 3 speed and muscle memory. Modern Rust safety and intelligence.
</p>

<p align="center">
  <a href="https://lcajigasm.github.io/lux/"><img src="https://img.shields.io/badge/website-lcajigasm.github.io%2Flux-ff9e3b.svg" alt="Website"></a>
  <a href="https://github.com/lcajigasm/lux/releases/latest"><img src="https://img.shields.io/github/v/release/lcajigasm/lux?color=brightgreen&label=release" alt="Release"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License"></a>
  <a href="https://github.com/lcajigasm/lux/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/lcajigasm/lux/ci.yml?branch=main&label=CI" alt="CI"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-2024%20edition-orange.svg" alt="Rust 2024"></a>
  <a href="#privacy--core-philosophy"><img src="https://img.shields.io/badge/telemetry-zero-blueviolet.svg" alt="Zero Telemetry"></a>
</p>

---

## Overview

**Lux** is a fast, free, open-source native text and code editor built from the ground up in Rust. It is designed to recapture and surpass the legendary ergonomics of **Sublime Text 3**: instant cold startup (< 150ms), imperceptible input latency (< 8ms), seamless editing of massive files, first-class multi-cursor workflows, and an unobtrusive, distraction-free user interface.

🌐 **Website & Documentation:** [https://lcajigasm.github.io/lux/](https://lcajigasm.github.io/lux/)
Publishing that site is manual: `.github/workflows/pages.yml` runs only on `workflow_dispatch`. A push to `main` no longer deploys it, because the Pages API returned HTTP 422 for this private repository (run 41274477945).

Unlike web-based or Electron editors, Lux is compiled directly to native machine code with hardware-accelerated WGPU rendering. It incorporates an in-process Git engine (`gix`), an integrated PTY terminal (`portable-pty`), a non-blocking Language Server Protocol client (`async-lsp`), tree-sitter incremental syntax highlighting, and sandboxed WebAssembly plugins (`wasmtime`).

Target platforms, in order: **macOS**, **Windows**, **Linux**, then **iPadOS/iOS**, then **Android**.

---

## Downloads

Lux is a macOS editor first. Linux and Windows still build, and they are secondary.

Pushing a `vX.Y.Z` tag runs [`.github/workflows/release.yml`](.github/workflows/release.yml) on `ubuntu-latest` and attaches the Linux packages plus `SHA256SUMS`. That publish starts [`.github/workflows/release-macos.yml`](.github/workflows/release-macos.yml) once: one `macos-14` runner, 20 minute cap, universal `.dmg`, attached to the same release. That job appends the `.dmg` line to `SHA256SUMS`. The macOS workflow does not run on pull request, branch push, or the tag push itself, so the Linux workflow does not start a second macOS job. `workflow_dispatch` is only a manual retry. The repository is private, so a GitHub session is required to download a release. On a private repository each macOS minute is billed at 10×.

| Asset | Where it is built |
|---|---|
| `lux-macos-universal.dmg` | `macos-14`, when the GitHub Release is published (`scripts/package-macos.sh`) |
| `lux-linux-x86_64`, `lux-linux-amd64.deb`, `SHA256SUMS` | GitHub Actions, `ubuntu-latest`, on a `v*` tag. The macOS job then adds the `.dmg` hash to `SHA256SUMS` |
| `lux-windows-x86_64.msi` | outside CI, `scripts/package-msi.ps1` (unsigned WiX) |

### Install on macOS

The disk image contains `Lux.app` (bundle id `dev.lux.editor`), a universal binary (Apple Silicon and Intel, `lipo`), `lux.icns`, and an **ad-hoc** signature. It is not notarized. There is no Apple Developer ID certificate in this repo, and Gatekeeper will quarantine the app.

1. Open `lux-macos-universal.dmg` and drag Lux to Applications.
2. If macOS reports the app as damaged or from an unidentified developer, clear quarantine and open it again:

```bash
xattr -dr com.apple.quarantine /Applications/Lux.app
```

`lux .` in a terminal opens the current folder. The bundle lists `public.folder`, so Finder can hand a folder to Lux. See [docs/macos-parity.md](docs/macos-parity.md).

The `.deb` depends on `libfontconfig1` and `libegl1` (the GPU loader the Linux binary dlopens), plus the xkbcommon, X11, and Wayland libraries it loads before it opens a window. No signing certificate is stored in this repo. The [v1.1.0 macOS disk image](https://github.com/lcajigasm/lux/releases/download/v1.1.0/Lux.dmg) is the older Apple Silicon installer, not the universal image above. See [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/STATUS.md](docs/STATUS.md).

---

## Highlights & Features

### ⚡ Sublime Text 3 Editing & Multi-Cursor Ergonomics
- **Quick Add Next Occurrence (`Cmd+D` / `Ctrl+D`)**: Selects word under cursor or appends the next identical match as an independent cursor.
- **Skip Occurrence (`Cmd+K, Cmd+D`)**: Deselects the current match and advances to the next occurrence.
- **Select All Occurrences (`Ctrl+Cmd+G` / `Alt+F3`)**: Instantly creates cursors across all matching occurrences in the buffer.
- **Split Selection into Lines (`Cmd+Shift+L` / `Ctrl+Shift+L`)**: Splits multi-line selections into individual line-end cursors.
- **Column / Rectangular Selection (`Option+Drag` / `Alt+Drag`)**: Pixel-accurate vertical column cursor placement; extend cursors vertically via `Cmd+Option+Up/Down`.
- **Parallel Multi-Cursor Clipboard**: Copying $N$ lines with $N$ cursors pastes line $i$ into cursor $i$ rather than duplicating all lines at each cursor.
- **Smart Selection Expansion (`Cmd+L`, `Cmd+Shift+Space`)**: Expand selection to line (`Cmd+L`), or expand to enclosing scope / delimiters (`Cmd+Shift+Space`).
- **Line Manipulation**:
  - Swap line up / down (`Cmd+Ctrl+Up/Down` or `Ctrl+Shift+Up/Down`).
  - Duplicate line / selection (`Cmd+Shift+D` / `Ctrl+Shift+D`).
  - Join lines (`Cmd+J` / `Ctrl+J`) with whitespace collapse.
  - Delete line (`Cmd+Shift+K` / `Ctrl+Shift+K`) without altering clipboard.
- **Code Folding**: Triangular gutter indicators (`▼`/`▶`), folded inline pills (`⋯`), fold current block (`Cmd+Option+[`), unfold (`Cmd+Option+]`), and fold by level (`Cmd+K, Cmd+1..5`).
- **Auto-Pairs & Delimiter Navigation**: Auto-closing pairs (`()`, `[]`, `{}`, `""`, `''`), smart step-over, selection wrapping, and bracket jump (`Ctrl+M`).
- **Clean Whitespace on Save**: Automated trailing whitespace trimming with active cursor line protection (`editor.trimTrailingWhitespacePreserveCursor`) and trailing newline enforcement.

### 🧭 Navigation & Project Search
- **Goto Anything (`Cmd+P` / `Ctrl+P`)**: Instant fuzzy file matching with real-time transient buffer preview.
  - `:line[:col]` jump to exact line and column (e.g., `main.rs:120:15`).
  - `@symbol` filter functions, structs, and symbols within the active file.
  - `#term` jump to occurrences within the file.
- **Goto Symbol in Project (`Cmd+Shift+R`)**: Workspace-wide background symbol index covering all project roots.
- **Bookmarks (`Cmd+F2`)**: Cyan gutter ribbons, sequential cycling (`F2` / `Shift+F2`), and multi-cursor selection of all bookmarks (`Option+F2`).
- **Interactive Find & Replace (`Cmd+F` / `Cmd+H`)**: In-buffer regex search bar, case sensitivity, whole word, match counter, and minimap match indicators.
- **Project Find in Files (`Cmd+Shift+F`)**: Ripgrep-powered workspace grep streaming matches into an interactive, editable "Find Results" virtual buffer with `F4` jump-to-source navigation.

### 📐 Multi-Pane Split Layouts
- **Preconfigured Grid Layouts (`Cmd+Option+1..5`)**:
  - 1 Column (Single view)
  - 2 Columns (Vertical split)
  - 3 Columns
  - 2 Rows (Horizontal split)
  - 2x2 Grid (4-pane quadrant)
- **Pane Management**: Navigate between groups (`Ctrl+1..4`), move tabs across groups (`Ctrl+Shift+1..4`), and resize with draggable pane dividers.
- **Windows / Linux**: the keymap binds two columns to `Alt+Shift+2` and two rows to `Alt+Shift+8`. `Ctrl+Alt+2` is an extra split-right chord. AltGr text (for example `€`) is still inserted.

### 🧩 Extensible Snippets & Keyboard Macros
- **Sublime Text Snippets**: Supports native `.sublime-snippet` XML and JSON formats. Interactive tabstop navigation (`Tab` / `Shift+Tab`), placeholder defaults (`${1:default}`), and real-time synchronized mirror fields.
- **Snippet Picker (`Cmd+Option+S`)**: Fuzzy searchable popup showing triggers, descriptions, and syntax scopes.
- **Keyboard Macros**:
  - Record macro (`Ctrl+Q`) with coral `● Recording Macro` status bar indicator.
  - Multi-cursor playback (`Cmd+Shift+Q`).
  - Sublime `.sublime-macro` JSON schema compatibility; save macros to `~/.lux/macros/`.

### 🎨 Themes & Distraction Free Mode
- **Authentic Sublime Color Schemes**: Pixel-perfect ports of **Monokai**, **Mariana**, **Breakers**, and **Celeste**, with WCAG AA compliance.
- **Live Theme Picker (`Preferences: Color Theme`)**: Real-time theme preview with color swatches.
- **Distraction Free Mode (`Shift+F11` / `Cmd+Ctrl+Shift+F`)**: Clean fullscreen writing mode with centered text column, hiding tab bar, status bar, sidebar, minimap, and gutters.

### 🛠️ Developer Tooling & Deep Integration
- **In-Process Git (`lux-git`)**: Native `gix` integration (zero shell-out overhead). Real-time gutter change indicators, floating diff hunk cards, inline stage/revert actions, and 3-way merge conflict resolution.
- **LSP Client (`lux-lsp`)**: Non-blocking language server integration with diagnostic squiggles, autocompletion popup, markdown hover info, Go to Definition (`F12`), Find References (`Shift+F12`), and Problems dock.
- **Integrated PTY Terminal (`lux-term`)**: Native terminal panel (`Cmd+\``) with ANSI 256/truecolor support, multiple named shell tabs, background tasks panel with cancellation, and clickable compiler error jump links.
- **Build System Runner**: Build (`Cmd+B`), Build With... (`Cmd+Shift+B`), automatic detection for Cargo, `package.json`, and custom build definitions, streaming output, and error navigation (`F4`/`Shift+F4`).
- **Sandboxed WASM Plugins (`lux-plugin`)**: Wasmtime Component Model host with fuel consumption caps, memory quotas, and explicit user permission prompts.
- **Accessibility & i18n**: AccessKit node hierarchy with VoiceOver live announcer (`ANNOUNCER_ID`), F6 focus ring navigation, and complete English/Spanish localization.
- **Workspace Projects**: `.sublime-project` and `.lux-project` multi-root workspace support with Quick Switch Project (`Cmd+Ctrl+P`).
- **Settings Importers**: Layered JSONC configuration (`~/.config/lux/config.jsonc`) with hot-reload and one-click import from VS Code, Sublime Text 3, and JetBrains.

---

## Keyboard Shortcuts Quick Reference

| Action | macOS Shortcut | Windows / Linux Shortcut |
|---|---|---|
| **Goto Anything** | `Cmd+P` | `Ctrl+P` |
| **Command Palette** | `Cmd+Shift+P` | `Ctrl+Shift+P` |
| **Goto Symbol in Project** | `Cmd+Shift+R` | `Ctrl+Shift+R` |
| **Quick Add Next Occurrence** | `Cmd+D` | `Ctrl+D` |
| **Skip Occurrence** | `Cmd+K, Cmd+D` | `Ctrl+K, Ctrl+D` |
| **Select All Occurrences** | `Ctrl+Cmd+G` | `Alt+F3` |
| **Split Selection into Lines** | `Cmd+Shift+L` | `Ctrl+Shift+L` |
| **Column Selection** | `Option+Drag` | `Alt+Drag` |
| **Swap Line Up / Down** | `Cmd+Ctrl+Up / Down` | `Ctrl+Shift+Up / Down` |
| **Duplicate Line / Selection** | `Cmd+Shift+D` | `Ctrl+Shift+D` |
| **Join Lines** | `Cmd+J` | `Ctrl+J` |
| **Delete Line** | `Cmd+Shift+K` | `Ctrl+Shift+K` |
| **Fold / Unfold Code** | `Cmd+Option+[ / ]` | `Ctrl+Shift+[ / ]` |
| **Toggle Bookmark** | `Cmd+F2` | `Ctrl+F2` |
| **Next / Previous Bookmark** | `F2` / `Shift+F2` | `F2` / `Shift+F2` |
| **Select All Bookmarks** | `Option+F2` | `Alt+F2` |
| **Find in Buffer** | `Cmd+F` | `Ctrl+F` |
| **Replace in Buffer** | `Cmd+Option+F` / `Cmd+H` | `Ctrl+H` |
| **Find in Files (Project)** | `Cmd+Shift+F` | `Ctrl+Shift+F` |
| **Next / Prev Build/Find Match**| `F4` / `Shift+F4` | `F4` / `Shift+F4` |
| **Layout 1 / 2 / 3 / 4 / Grid**| `Cmd+Option+1..5` | `Alt+Shift+1..5` |
| **Focus Pane Group** | `Ctrl+1..4` | `Ctrl+1..4` |
| **Build Default / Build With** | `Cmd+B` / `Cmd+Shift+B` | `Ctrl+B` / `Ctrl+Shift+B` |
| **Record / Playback Macro** | `Ctrl+Q` / `Cmd+Shift+Q` | `Ctrl+Q` / `Ctrl+Shift+Q` |
| **Insert Snippet** | `Cmd+Option+S` | `Ctrl+Alt+S` |
| **Distraction Free Mode** | `Shift+F11` / `Cmd+Ctrl+Shift+F` | `Shift+F11` |
| **Toggle Terminal** | `Cmd+\`` | `Ctrl+\`` |
| **Cycle Focus Ring** | `F6` / `Shift+F6` | `F6` / `Shift+F6` |

---

## Performance Targets & Measured Benchmarks

Performance in Lux is treated as a core correctness requirement tested continuously with Criterion benchmarks:

| Metric | Budget Target | Measured Status |
|---|---|---|
| **Cold Startup Time** | $< 150\text{ ms}$ | **~62 ms** (Apple Silicon) |
| **Key-to-Pixel Input Latency** | $< 8\text{ ms}$ | **~2–4 ms** (120 Hz ProMotion supported) |
| **Memory Footprint (Base RSS)** | $< 50\text{ MB}$ | **~28 MB** (Empty window with chrome) |
| **Memory on 100 MB File** | $< 3\times\text{ file size}$ | **~1.6\times\text{ file size}$** (Rope chunking) |
| **Distribution Bundle Size** | $< 40\text{ MB}$ | **12.5 MB** DMG / **11.5 MB** archive |
| **Background CPU Idle** | $0.0\%\text{ CPU}$ | **0.0% CPU** (Purely event-driven, zero polling) |

---

## Building from Source

### Prerequisites
- **Rust Toolchain**: Rust 1.97+ (see [`rust-toolchain.toml`](rust-toolchain.toml))
- **Platform Dependencies**:
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `pkg-config`, `libasound2-dev`, `libwayland-dev`, `libxkbcommon-dev`
  - Windows: Visual Studio Build Tools with C++ toolchain

### Build Commands

```bash
# Clone repository
git clone https://github.com/lcajigasm/lux.git
cd lux

# Build in release mode
cargo build --release

# Run Lux
cargo run -p lux --release -- [file_or_directory]

# Execute full test suite (512+ unit and integration tests)
cargo test --workspace

# Run complete quality pipeline (formatting, clippy, architecture limits, core isolation)
cargo run -p xtask
```

### Packaging Release Artifacts

To produce a local release tree or native macOS `.dmg` and `.app` bundle:

```bash
./scripts/package.sh dist/lux
```

---

## Architectural Principles & Quality Rules

Lux is built according to strict architectural guidelines defined in [`CLAUDE.md`](CLAUDE.md):

1. **Nothing Simulated**: Every button, shortcut, menu item, and command connects to a complete, verifiable implementation. No stubs or mock bypasses.
2. **Core Isolation**: `lux-core` and domain crates have zero UI or graphics dependencies, enforced by [`scripts/check-core-isolation.sh`](scripts/check-core-isolation.sh).
3. **Strict Limits**: No file may exceed 600 lines, and no struct may exceed 25 fields, enforced by [`scripts/check-limits.sh`](scripts/check-limits.sh).
4. **Event-Driven Architecture**: Zero polling loops. Native file-system events, OS process hooks, and cross-thread channels wake the UI thread only when necessary.
5. **No I/O on UI Thread**: All disk I/O, ripgrep searches, git computations, and LSP communications execute on dedicated background worker threadpools.

### Workspace Crates

- [`crates/lux-core`](crates/lux-core): Text buffer rope model, transactions, selections, bookmarks, folding, and edits.
- [`crates/lux-render`](crates/lux-render): Hardware-accelerated text layout and WGPU rendering primitives.
- [`crates/lux-syntax`](crates/lux-syntax): Tree-sitter incremental parsing, scope highlighting, and theme definitions.
- [`crates/lux-workspace`](crates/lux-workspace): Multi-root workspaces, non-polling filesystem watcher, and project symbol indexing.
- [`crates/lux-git`](crates/lux-git): In-process Git engine powered by `gix`.
- [`crates/lux-lsp`](crates/lux-lsp): Non-blocking asynchronous Language Server Protocol client.
- [`crates/lux-term`](crates/lux-term): Native pseudo-terminal (PTY) emulation and shell integration.
- [`crates/lux-plugin`](crates/lux-plugin): Sandboxed WebAssembly Component Model plugin host (`wasmtime`).
- [`crates/lux-ui`](crates/lux-ui): Editor UI widgets, minimap, file tree, search bars, modals, AccessKit accessibility, and i18n.
- [`crates/lux-platform`](crates/lux-platform): Cross-platform windowing, input events, and clipboard abstractions.
- [`crates/lux-config`](crates/lux-config): JSONC configuration schema, hot-reloader, and settings migration.
- [`crates/lux-pack`](crates/lux-pack): Zero-telemetry crash reporting and Ed25519 update verification.
- [`crates/lux`](crates/lux): Main desktop entrypoint orchestrating all subsystem controllers.

---

## Privacy & Core Philosophy

In accordance with [`docs/NON-GOALS.md`](docs/NON-GOALS.md), Lux guarantees:
- **Zero Telemetry**: No tracking, phone-home requests, or analytics.
- **Zero Integrated LLM/Cloud Bloat**: No embedded chat widgets, forced AI autocomplete, or background cloud dependencies.
- **Zero In-Editor Debugger (DAP)**: Focused exclusively on high-performance code editing.
- **Local-First Plugins**: All plugins execute locally inside sandboxed WebAssembly containers without remote centralized marketplaces.

---

## License

Dual-licensed under either:
- **MIT License** ([`LICENSE-MIT`](LICENSE-MIT))
- **Apache License, Version 2.0** ([`LICENSE-APACHE`](LICENSE-APACHE))

at your option.
