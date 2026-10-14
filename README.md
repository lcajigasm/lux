# Lux

A fast, free, cross-platform text and code editor. The quality bar is
Sublime Text 3: instant startup, imperceptible input latency, flawless
handling of huge files, first-class multicursor, and a UI that never gets in
the way. Target platforms, in order: macOS, Windows, Linux, then iPadOS/iOS,
then Android.

MIT OR Apache-2.0. Free and open source. No paid tier, no telemetry by
default.

## Status (v1.0.0)

Lux is certified for **v1.0.0** desktop release. All domain crates are fully integrated
into the native application binary:
- **Fast, Transactional Editing**: Multicursor editing, atomic file saving, and tree-sitter incremental syntax highlighting with Lux Dark theme.
- **Sublime-Style UX & Parity**:
  - **Occurrence & Column Selection**: Quick Add Next (`Cmd+D`), Skip Occurrence (`Cmd+K, Cmd+D`), Select All Occurrences (`Ctrl+Cmd+G`), Column/Box selection (`Option+Drag`), and Split Selection into Lines (`Cmd+Shift+L`).
  - **Code Folding & Bracket Matching**: Gutter fold indicators, inline fold pills, Fold/Unfold by Level (`Cmd+K, Cmd+1..5`), auto-closing pairs with smart step-over/backspace, and matching delimiter highlights with `Control+M` jump.
  - **Goto Anything & Project Symbols**: Unified `Cmd+P` with `:line:col`, `@symbol`, `#term`, live transient preview with Escape restoration, and project-wide background symbol indexing (`Cmd+Shift+R`).
  - **Bookmarks**: Line bookmarks (`Cmd+F2`), sequential navigation (`F2`/`Shift+F2`), simultaneous selection into multicursors (`Option+F2`), and cyan gutter ribbons.
  - **Find & Replace**: In-buffer regex bar (`Cmd+F` / `Cmd+Option+F` / `Cmd+H`) with live counts and minimap overview ticks; ripgrep Find in Files (`Cmd+Shift+F`) with virtual editable "Find Results" buffer and `F4` hop.
  - **Grid & Multi-Pane Layouts**: Presets for 1-4 columns, 2-3 rows, and 2x2 grid (`Cmd+Alt+1..5`) with group focus/relocation (`Ctrl+1..4`), draggable dividers, and tab sync.
  - **Build System Runner**: Build (`Cmd+B`), Build With... variant modal (`Cmd+Shift+B`), streaming PTY output dock, and compiler error navigation (`F4`/`Shift+F4`).
  - **Sublime Projects**: `.sublime-project` and `.lux-project` workspace folders, excludes, custom build systems, and Quick Switch Project (`Cmd+Ctrl+P`).
  - **Iconic Color Themes**: Monokai, Mariana, Breakers, and Celeste with live swatch preview picker (`Preferences: Color Theme`).
  - **Distraction Free Mode**: Centered distraction-free writing column in borderless fullscreen (`Shift+F11` / `Cmd+Ctrl+Shift+F`).
  - **Whitespace Clean On Save**: Automated trailing whitespace trimming with cursor protection and EOF final newline enforcement.
  - **Extensible Snippets**: Sublime XML (`.sublime-snippet`) and JSON snippets, recursive folder loading, tabstops, real-time mirror sync, and modal picker (`Cmd+Alt+S`).
  - **Keyboard Macros**: Sublime `.sublime-macro` format, recording (`Ctrl+Q`), live `● Recording Macro` status bar indicator, multicursor playback (`Cmd+Shift+Q`), and `~/.lux/macros/` persistence.
- **Workspace & Search**: Ripgrep-powered find in files (`Cmd+Shift+F`), diff preview replacement, session persistence, and non-polling native OS file watchers.
- **In-Process Git**: Native `gix` integration with real-time gutter change indicators, diff hunk inspection cards, staging, commit execution, and inline merge conflict resolution.
- **Language Intelligence**: Non-blocking LSP client (`lux-lsp`) with diagnostic squiggles, autocompletion popup, hover documentation, and Problems panel dock.
- **Integrated Terminal**: Native PTY terminal dock (`Cmd+\``) with ANSI color rendering, multiple shell tabs, task runner, and clickable error links.
- **Sandboxed Plugins**: Wasmtime Component Model plugin host with fuel quotas and interactive capability permission prompts.
- **Accessibility & i18n**: Complete F6 focus navigation ring, full AccessKit accessibility tree with VoiceOver announcer, and 100% Spanish/English catalog coverage.
- **Preferences**: Layered JSONC settings with hot-reloading (`ConfigWatcher`), Preferences modal UI, and migration importers for VS Code, Sublime Text, and JetBrains.
- **Packaging & Diagnostics**: Zero-telemetry local crash log writer (`lux-pack`) and release distribution packager (`scripts/package.sh`).

## Building & Running

    cargo build --workspace
    cargo test --workspace
    cargo run -p xtask      # full CI verification pipeline

Run `cargo run -p lux -- [file_or_directory]` to launch the editor.
To package an unsigned release distribution tree:

    ./scripts/package.sh dist/lux

See `rust-toolchain.toml` for the pinned toolchain.

## Repository layout

- `crates/` — the Cargo workspace (`lux-core` through `lux-ui`, plus the
  `lux` desktop binary). See `docs/ARCHITECTURE.md`.
- `docs/adr/` — architectural decisions, with context and alternatives.
- `docs/STATUS.md`, `docs/NON-GOALS.md`, `docs/PERFORMANCE.md` — what works,
  what's explicitly out of scope, and the measured performance budgets.
- `GOALS.md` — the phase-by-phase roadmap.
- `docs/receipts/` — historical implementation and verification records.

## Contributing

Read `CLAUDE.md` first — it's the working agreement, not just AI-assistant
config. Conventional Commits, English, one coherent change per commit, PRs
against `main` with CI green.
