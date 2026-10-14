# Phase 11 & v1.0.0 Release Notes — Lux Desktop v1.0.0

Lux is officially certified for **v1.0.0** desktop release on macOS, with cross-platform source parity and CI validation for Windows and Linux, and native mobile host architectures for iPadOS/iOS and Android.

## Highlights & Capabilities

- **Core & Multi-Document Editing**: Transactional editing with single-step undo/redo, grapheme-aware multicursor, multi-document tab strip with modified indicators and close handling, snippet expansion with interactive tabstops.
- **Tree-sitter Syntax Highlighting**: Incremental tree-sitter highlighting across 18 compiled-in languages with Lux Dark token colors and visible viewport clipping.
- **Sublime UX Parity**:
  - Unified Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) with fuzzy filtering over all actions.
  - Goto Anything (`Cmd+P` / `Ctrl+P`) with fuzzy search and `:line` jump navigation.
  - In-buffer Find and Replace bar (`Cmd+F` / `Cmd+H`) with regex, whole word, match highlights, and transactional replacements.
  - File tree sidebar (`Cmd+B`) with virtualized tree, node expansion, and tab opening.
  - Minimap overview with line density scaling, active viewport indicator, and click-to-scroll.
  - Multi-cursor placement via `Option+Click`.
- **Project Search & Workspaces**: Workspace multi-root support (`.lux-project`), project-wide find in files (`Cmd+Shift+F`) powered by embedded ripgrep stream, and replace in project with diff preview.
- **Sessions & Watchers**: Workspace session persistence across restarts, non-polling native OS file watcher, automatic clean reload, and dirty conflict prompt.
- **In-Process Git (`lux-git`)**: Native `gix` integration (zero shell-outs): real-time gutter change indicators, floating diff hunk inspection cards with stage/revert buttons, file staging/committing, and inline merge conflict resolution.
- **LSP Client (`lux-lsp`)**: Non-blocking language server integration with diagnostics squiggles, autocompletion popup, hover markdown documentation, Go to Definition (`F12`), Find References (`Shift+F12`), and Problems panel dock.
- **Integrated PTY Terminal (`lux-term`)**: Full terminal panel (`Cmd+\``) with color ANSI VT grid, multiple named shell sessions, background tasks panel with cancellation, and clickable compiler error jump links.
- **Sandboxed WASM Plugins (`lux-plugin`)**: `wasmtime` Component Model plugins with fuel consumption caps, memory quotas, and interactive capability security prompts.
- **Accessibility & Focus**: Complete panel focus navigation ring (`F6` / `Shift+F6`, `Escape` untrap), full AccessKit node hierarchy with live screen-reader announcer (`ANNOUNCER_ID`) certified with VoiceOver.
- **Localization (i18n)**: 100% key completeness across Spanish and English catalogs with live language switching and bidirectional text layout support.
- **Configuration & Preferences**: Layered JSONC configuration (`LayeredConfig`) with live `ConfigWatcher` hot-reloading (fonts, themes, rulers), Preferences modal UI, and settings migration importers for VS Code, Sublime Text, and JetBrains.
- **Local Crash Reporting & Updates**: Zero-telemetry local crash log writer (`lux-pack`) and Ed25519 digital signature verification for update payloads.
- **Packaging**: Production desktop distribution layout generator (`scripts/package.sh`), Windows Package Manager manifest (`packaging/winget/Lux.Lux.yaml`), compressed binary size 11.7 MB (< 40 MB budget).

## Quality & Gate Verification

- All workspace tests passing: `cargo test --workspace` (82 unit/integration tests in `lux`).
- All linter and compiler checks passing with zero warnings: `cargo clippy --workspace --all-targets -- -D warnings`.
- Strict architecture limits verified: all files $\le 600$ lines, all structs $\le 25$ fields ([`scripts/check-limits.sh`](../../scripts/check-limits.sh)).
- Core isolation verified: [`scripts/check-core-isolation.sh`](../../scripts/check-core-isolation.sh).
- Dependencies and licenses verified: `cargo deny check`.
- Criterion benchmarks passing with 100% success rate across all phase budgets.
