# Lux — Integration & Completion Roadmap (Agent `/goal` Backlog)

> **Authority & Status Context (Updated October 2026):**
> This document is the actionable backlog to bring Lux from crate-level implementations to a fully finished, verified, and shippable product across all target phases (v1.0.0 desktop, v1.1.0 iOS, v1.2.0 Android).
>
> Prior to executing any task, consult:
> - [`CLAUDE.md`](CLAUDE.md) — 10 Non-negotiable architectural rules (no simulated features, no UI thread I/O, no polling, closed scope, file/struct limits).
> - [`docs/STATUS.md`](docs/STATUS.md) — Ground truth of what is actually running per platform.
> - [`docs/AUDIT.md`](docs/AUDIT.md) — Implementation audit and confirmed integration gaps.
> - [`docs/NON-GOALS.md`](docs/NON-GOALS.md) — Strictly out of scope (no DAP/debugger, no AI/LLM, no cloud sync, no telemetry, no remote marketplace).
> - [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) — Enforced performance budgets and benchmark gates.
> - [`docs/adr/`](docs/adr/) — Architecture Decision Records (ADRs 0001 through 0017).

---

## How to Work on This Backlog

1. **Sequential execution:** Select the lowest numbered `OPEN` goal whose dependencies (`Depends`) are `DONE`.
2. **Launch with `/goal`:** Use the paste template below:
   ```text
   /goal Execute TODO.md item GNN (<title>). Follow CLAUDE.md, docs/STATUS.md, and docs/AUDIT.md.
   Ensure all changes compile, pass cargo run -p xtask (or equivalent formatting, clippy, limits, isolation checks),
   and verify on the running application before marking complete.
   ```
3. **No simulated behavior (Rule #1):** A feature is either completely wired and operational or it does not exist in the UI. Never add buttons, menu items, or views that do not execute real logic.
4. **Done means STATUS moves:** Updating code or crate tests is not enough; the feature must run in the `lux` binary and update `docs/STATUS.md` with supporting evidence or receipts in `docs/receipts/`.

**Item States:** `OPEN` · `BLOCKED` · `DONE`

---

## Track 0 — Foundations & Core Editing Baseline

### G00 — Sync Roadmap, Status & Integration Backlog
- Status: `DONE`
- Depends: —
- Goal: Maintain alignment between `GOALS.md`, `docs/AUDIT.md`, `docs/STATUS.md`, and this backlog.
- Evidence: `docs/AUDIT.md` reopened on 2026-09-08 and updated 2026-09-28; `TODO.md` established as the master closure backlog.

### G01 — Cross-Platform System Monospace Font Discovery
- Status: `DONE` (macOS verified; Linux/Windows source ready, runtime proof tracked in G22/G23)
- Depends: G00
- Goal: `crates/lux/src/font.rs` discovers and validates system monospace fonts across macOS (`SF Mono`, `Menlo`), Windows (`Consolas`, `Cascadia Code`), and Linux (`DejaVu Sans Mono`, `Liberation Mono`, fontconfig fallback).
- Evidence: `crates/lux/src/font.rs` rejects invalid font bytes and loads real system fonts.

### G02 — Desktop Document Lifecycle & Atomic Storage
- Status: `DONE` (macOS)
- Depends: G01
- Goal: The desktop binary opens paths from argv or native open dialog (`Cmd/Ctrl+O`), saves (`Cmd/Ctrl+S`), Save As (`Cmd/Ctrl+Shift+S`), creates new documents (`Cmd/Ctrl+N`), and confirms dirty close (`Cmd/Ctrl+W` / close button) with parent modal dialogs.
- Evidence: `crates/lux/src/document.rs`, `crates/lux-core/src/buffer/save.rs` with atomic save, unique temp files, backup files, symlink and permission preservation.

### G03 — Configurable Keymaps & Core Editing Operations
- Status: `DONE` (macOS)
- Depends: G02
- Goal: `lux-config` Sublime keymap preset dispatches command chords to core editing operations (cut, copy, paste, undo, redo, select all, duplicate line, delete line, join lines, indent, dedent, toggle line comment).
- Evidence: `crates/lux/src/input.rs` and `crates/lux-config/src/keymap/binding.rs`.

### G04 — Tree-sitter Syntax Highlighting in Viewport
- Status: `DONE` (macOS)
- Depends: G02, G03
- Goal: `lux-syntax` tree-sitter incremental parsing colors visible glyphs using `LUX_DARK` token colors in `crates/lux/src/render.rs`, updated on document open and buffer edits.
- Evidence: `crates/lux/src/app.rs` (`update_syntax_on_open`, `sync_syntax_on_edit`), `crates/lux/src/render.rs`.

### G05 — Snippet Expansion & Interactive Placeholder Navigation
- Status: `DONE` (macOS)
- Depends: G03
- Goal: `lux-core::snippets` expansion with tabstop placeholders; `Tab` / `Shift+Tab` cycles forward/backward; typing replaces mirror placeholders; `Escape` exits session.
- Evidence: `crates/lux/src/input.rs` (`expand_snippet`, `insert_at_cursor`, `NamedKey::Tab`).

---

## Track 1 — Desktop Multi-Document & Chrome Layout

### G06 — Real Multi-Document Architecture & Rendered Tab Bar
- Status: `DONE` (macOS verified; all workspace tests passing)
- Depends: G02, G05
- Goal: Transition the desktop application from a single buffer (`App.buffer`) to managing multiple opened documents in memory, with an interactive, rendered tab bar above the editor.
- Evidence:
  - `crates/lux/src/document.rs`: `Documents` tracks collection of `DocumentEntry` records (each with independent buffer, history, cursors, scroll position, and dirty state) with tab switching, cycling, closing, and reopen stack.
  - `crates/lux/src/document/io.rs`: Background worker I/O cleanly separated, keeping files under the 600-line limit.
  - `crates/lux/src/render.rs`: Tab bar rendered at window top with active tab accent, inactive tab styling, tab title text, modified dirty indicator, and close button. Editor view offset below tab bar.
  - `crates/lux/src/app.rs`: Hit-testing chrome clicks (tab switching, close button, middle click closing) and cycling shortcuts.
  - `crates/lux-ui/src/tabs.rs`: `TabBar::set_title` to dynamically update tab titles on save/rename.
  - `crates/lux/src/document/tests.rs`: Integration tests for opening multiple files, switching tabs with preserved buffer/history/dirty state, duplicate-open tab activation, closing without app exit, and reopen stack.
- Done when:
  - `Documents` manages a collection of open document states (each having its own `Buffer`, `History`, `MultiCursor`, file path, and dirty state).
  - Opening a second file creates a new tab rather than replacing the current document.
  - `lux-ui::tabs::TabBar` is rendered in `crates/lux/src/render.rs` with active tab styling, modified dot for dirty buffers, and close buttons.
  - Mouse clicks on tabs switch active document; clicking tab close (or middle click) closes tab with dirty confirmation.
  - Keyboard shortcuts (`Ctrl/Cmd+Tab`, `Ctrl/Cmd+Shift+Tab`, `Ctrl/Cmd+W`) switch and close tabs; reopening last closed tab (`Ctrl/Cmd+Shift+T`) restores buffer via `TabBar::reopen_last_closed`.
  - Switching tabs updates viewport, cursor, syntax highlighting, and window title instantaneously.
- Verify:
  - Integration tests in `crates/lux/src/document/tests.rs` verifying multi-document lifecycle.
  - Native rendering and tab switching verification on macOS/Linux.
- Out of scope: Detached floating tab windows or arbitrary complex drag-and-drop tab tearing (keep within `lux-ui` tab strip model).

### G07 — Rendered Status Bar & Interactive Command Segments
- Status: `OPEN`
- Depends: G06
- Goal: Render the status bar (`lux-ui::status_bar`) at the bottom of the window in `crates/lux/src/render.rs` with real, clickable segments.
- Done when:
  - Status bar is laid out and drawn at the bottom edge of the window with contrast-compliant theme styling.
  - Displays real buffer telemetry: cursor line & column (`Ln X, Col Y`), detected syntax language, file encoding (UTF-8, UTF-16, etc.), EOL format (LF / CRLF), and indentation style (Spaces: 4 / Tabs).
  - Clicking on each item triggers its corresponding action:
    - Cursor position -> opens Goto Line prompt.
    - Language -> opens Language Mode selector.
    - Encoding -> prompts to re-encode / save with encoding.
    - EOL -> toggles LF / CRLF.
    - Indent -> toggles tab size / spaces vs tabs.
  - Status bar updates immediately when cursor moves or buffer properties change.
- Verify:
  - `cargo test -p lux-ui`
  - Interactive verification in `cargo run -p lux`.
- Out of scope: Third-party widget extensions in the status bar beyond the standard segments.

### G08 — Unified Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`)
- Status: `OPEN`
- Depends: G06, G07
- Goal: Wire and render `lux-ui::command_palette` into the desktop binary with fuzzy ranking and keyboard execution.
- Done when:
  - `Cmd+Shift+P` (macOS) / `Ctrl+Shift+P` (Windows/Linux) toggles the floating command palette overlay centered near the top of the editor.
  - All application commands (file operations, editing commands, view toggles, settings) are registered in `CommandRegistry`.
  - Typing filters commands in real-time using `lux-search::fuzzy` with matched character highlighting.
  - `ArrowUp` / `ArrowDown` navigates suggestions; `Enter` executes the highlighted command; `Escape` closes the palette and restores editor focus.
  - Commands respect `When` clause contexts (e.g. `editorFocus`, `hasSelection`).
- Verify:
  - Unit tests for command palette dispatch in `crates/lux/tests/`.
  - Palette open, filter, and action execution in running app.
- Out of scope: Nested multi-step interactive wizard inputs unless required by a specific command.

### G09 — Goto Anything (`Cmd+P` / `Ctrl+P`) with Line & Symbol Jumps
- Status: `OPEN`
- Depends: G08
- Goal: Integrate `lux-ui::goto_anything` for fast fuzzy file navigation and location jumping.
- Done when:
  - `Cmd+P` (macOS) / `Ctrl+P` (Windows/Linux) opens the Goto Anything modal.
  - Lists open tabs, recently opened files, and workspace files (if a workspace is open).
  - Typing queries matches filenames fuzzily using `nucleo` (`lux-search::fuzzy`).
  - Sublime-style suffix `:line` (e.g. `main.rs:42`) jumps directly to the specified line number upon opening.
  - Selected file opens into a tab in the editor; if already open, activates its existing tab.
- Verify:
  - Headless tests for query parsing, file ranking, and jump location resolution.
  - Interactive verification in running app.
- Out of scope: Arbitrary full-text search inside the Goto Anything input (handled by Find in Files).

### G10 — In-Buffer Find & Replace Bar (`Cmd+F` / `Cmd+H`)
- Status: `OPEN`
- Depends: G04, G06
- Goal: Wire and paint `lux-ui::search_bar` for in-buffer search and replacement.
- Done when:
  - `Cmd+F` / `Ctrl+F` reveals the search bar; `Cmd+H` / `Ctrl+H` reveals search and replace fields.
  - Search queries highlight all match occurrences in the buffer with distinct theme colors in `render.rs`.
  - Displays match count (`X of Y matches`).
  - `Enter` / `Shift+Enter` (and next/prev buttons) cycle the primary cursor through matches, scrolling the viewport to reveal each match.
  - Match case, whole word, and regex toggle options work accurately using `lux-core::find_replace`.
  - Replace button and Replace All button perform transactional edits with undo/redo support.
  - `Escape` dismisses the search bar and returns focus to the editor.
- Verify:
  - Automated tests for search bar state, navigation, and replacement transactions.
  - Visual verification of match highlights and scrolling.

### G11 — Minimap & Syntax-Driven Code Folding
- Status: `OPEN`
- Depends: G04, G06
- Goal: Paint `lux-ui::minimap` and enable Tree-sitter code folding in the editor gutter.
- Done when:
  - Minimap renders a scaled-down overview of buffer lines on the right side of the editor with current viewport indicator.
  - Clicking or dragging the minimap scrolls the editor viewport smoothly.
  - Tree-sitter AST queries identify foldable ranges (functions, blocks, comments, classes).
  - Folding arrows appear in the gutter; clicking a fold collapses the line range and renders a fold placeholder (`...`).
  - Hidden lines are omitted from virtualized text rendering, and cursor movement skips folded blocks properly.
- Verify:
  - `cargo test -p lux-ui` and `cargo test -p lux-syntax`.
  - Visual rendering of minimap and fold toggles.

---

## Track 2 — Workspace, File Tree & Project Search

### G12 — Workspace Directory Open, Virtualized File Tree & Native Watchers
- Status: `OPEN`
- Depends: G06, G08
- Goal: Support opening folders/projects (`lux-workspace`), render a virtualized file explorer tree (`lux-ui::file_tree`), and keep file status fresh using non-polling native OS watchers.
- Done when:
  - Launching `lux <directory>` or invoking "Open Folder" sets the active workspace root.
  - File tree sidebar renders folder hierarchy with expandable directories, file icons, and indentation guides.
  - Clicking a file opens it in a tab; keyboard navigation (`Up`/`Down`/`Enter`/`Left`/`Right`) navigates the tree.
  - File creation, deletion, and renaming in the tree update disk transactionally via `trash` and filesystem operations.
  - Background `FileWatcher` (`notify`-based) captures external disk additions/deletions/modifications and updates tree state without UI thread polling or disk thrashing.
  - Ignored patterns (`.git`, `target`, `node_modules`, `.DS_Store`) are excluded from recursive indexing by default.
- Verify:
  - `cargo test -p lux-workspace` and `cargo test -p lux-ui`.
  - Automated test verifying tree updates upon external file addition/deletion.
- Out of scope: Remote SFTP / cloud virtual filesystems (NON-GOALS).

### G13 — Project-Wide Find in Files & Replace with Diff Preview
- Status: `OPEN`
- Depends: G12
- Goal: Integrate `lux-search::find_in_files` (embedded ripgrep engine) and project replace preview into a workspace search panel.
- Done when:
  - `Cmd+Shift+F` / `Ctrl+Shift+F` opens the project search panel.
  - Queries stream results progressively across files without blocking the UI thread or freezing the frame rate.
  - Results show file paths, line numbers, and formatted contextual snippets with match highlights.
  - Clicking a search result opens the file and reveals the exact match line and column in the editor.
  - Project replace computes a `ReplacementPreview` (`lux-search::replace_preview`) showing diff hunks before applying.
  - User can review/uncheck individual hunks and click "Apply" to atomically execute changes across all target files.
- Verify:
  - `cargo test -p lux-search`.
  - Benchmark performance adheres to the budget: search over 1 GB corpus in < 2 s (`docs/PERFORMANCE.md`).

### G14 — Workspace Session Persistence & External Change Conflicts
- Status: `OPEN`
- Depends: G06, G12
- Goal: Persist workspace sessions across application restarts and cleanly resolve external file change conflicts.
- Done when:
  - Exiting the application saves the workspace state (open tab paths, active tab, cursor positions, scroll offsets) to `.lux-project` or local session cache.
  - Reopening the folder restores the exact prior tab layout and cursor positions seamlessly.
  - When an opened file is modified externally on disk:
    - If the buffer is clean (unmodified), it auto-reloads with the new disk content.
    - If the buffer is dirty (unsaved edits exist), a non-blocking conflict bar prompts the user: "File modified on disk: [Compare Diff] [Overwrite Disk] [Reload Disk]".
- Verify:
  - Test session serialization and deserialization.
  - Regression test for external file conflict detection and resolution.

---

## Track 3 — Subsystem GUI Integration (Git, LSP, Terminal, Plugins)

### G15 — In-Process Git Integration (`lux-git`)
- Status: `OPEN`
- Depends: G06, G12
- Goal: Wire `lux-git` (`gix`-based, zero shell-outs) into the desktop GUI surfaces.
- Done when:
  - Gutter displays color-coded change indicators (added, modified, deleted line hunks) in real time against `HEAD`.
  - Clicking a gutter indicator reveals a popup diff hunk with options to stage or revert that specific hunk.
  - Source Control sidebar panel displays repository status: current branch, changed files categorized by staged/unstaged, and uncommitted modifications.
  - Diff view (`lux-git::diff_view`) renders side-by-side or inline comparisons for modified files.
  - Basic staging, unstaging, commit message input, commit execution, and branch checkout operate cleanly from the UI.
  - Merge conflict markers in open buffers are detected and surfaced with inline resolution buttons ("Accept Current", "Accept Incoming", "Accept Both").
- Verify:
  - `cargo test -p lux-git`.
  - Verification on a real git repository with staged and unstaged hunks.
- Out of scope: Remote GitHub pull request review flows or hosting platform integrations (NON-GOALS).

### G16 — Real LSP Client Integration (`lux-lsp`)
- Status: `OPEN`
- Depends: G04, G06, G08, G12
- Goal: Integrate `lux-lsp` into the desktop shell, providing language intelligence without UI blocking.
- Done when:
  - Language servers (e.g. `rust-analyzer`, `typescript-language-server`, `pyright`, `gopls`) are discovered and launched on background workers based on open buffer file types.
  - Incremental document synchronization (`didOpen`, `didChange`, `didSave`, `didClose`) keeps server AST synchronized with editing.
  - Diagnostics (errors, warnings) render as inline squiggles in the viewport and populate the Problems panel (`FocusTarget::Problems`).
  - Autocompletion popup appears on typing or `Ctrl+Space`, showing sorted completion items with type icons; selecting an item shows documentation and resolves edits/imports.
  - Code navigation: "Go to Definition" (`F12`), "Find References" (`Shift+F12`), and "Workspace Symbols" open target locations accurately.
  - Hover tooltips display markdown documentation and type signatures.
  - "Format Document" / "Format on Save" applies server edits transactionally via `lux-core`.
  - Cross-file rename (`F2`) displays a preview of affected files and executes atomic `WorkspaceEdit`.
- Verify:
  - `cargo test -p lux-lsp` (including real-server integration tests against installed servers).
  - Verify zero UI thread stalls during large server responses or indexing.
- Out of scope: Debugger / DAP support, breakpoints, call stack inspection (strict NON-GOALS).

### G17 — Integrated PTY Terminal Panel (`lux-term`)
- Status: `OPEN`
- Depends: G06, G07
- Goal: Wire `lux-term` (`portable-pty` + `alacritty_terminal`) into a bottom dock terminal panel.
- Done when:
  - `Ctrl+\`` / `Cmd+\`` (or menu) toggles the bottom terminal panel.
  - Spawns real user shells (zsh, bash, fish, pwsh, cmd) via PTY; ConPTY on Windows.
  - Terminal grid renders correctly using the GPU text pipeline; supports color ANSI sequences and high throughput (100 MB/s without frame degradation).
  - Terminal input forwards keyboard events directly to the PTY child process when focused.
  - Window and panel resize updates PTY dimensions dynamically (`PtySize`).
  - Supports multiple named terminal tabs, split shells, and custom shell profiles.
  - Scrollback search, mouse selection, and clipboard copy function reliably.
  - Error and file links in output (`path/to/file:line:col`) are underlined and clickable, jumping directly to the file and line in the editor.
  - Closing terminal kills child processes cleanly without leaving zombie/orphan processes.
  - Tasks panel (`lux-term::tasks::TaskPanel`) allows running project commands with a cancel button.
- Verify:
  - `cargo test -p lux-term`.
  - Verify interactive shell commands (`top`, `vim`, `git status`) inside the running panel.
- Out of scope: Terminal panel on mobile platforms (explicitly excluded by ADR 0006).

### G18 — Sandboxed WASM Plugin Host (`lux-plugin`)
- Status: `OPEN`
- Depends: G08, G12
- Goal: Integrate `lux-plugin` (`wasmtime` Component Model) allowing plugins to extend commands, themes, and grammars with strict capability isolation.
- Done when:
  - Local plugin bundles can be installed from a folder or `.wasm` package.
  - Security model: displays an explicit permission grant dialog requesting required capabilities (Filesystem, Network, Terminal) before running.
  - Wasmtime runtime executes plugins with fuel-based CPU consumption limits and memory caps; plugins exceeding quotas are safely terminated.
  - Installed plugins can register custom commands into the Command Palette and provide custom themes and formatters.
  - 2 sample plugins verified working inside the desktop binary.
- Verify:
  - `cargo test -p lux-plugin` including sandbox-escape rejection tests.
  - Example plugin installation and command execution via palette.
- Out of scope: Hosted remote plugin store / extension marketplace (NON-GOALS).

---

## Track 4 — Focus Ring, Accessibility, i18n & Settings

### G19 — Full Keyboard Navigation & F6 Focus Ring
- Status: `OPEN`
- Depends: G06, G07, G12, G15, G17
- Goal: Implement complete panel keyboard navigation using the F6 ring without dead ends or traps.
- Done when:
  - `F6` cycles forward and `Shift+F6` cycles backward through visible surfaces:
    `Editor -> Command Palette -> Search -> File Tree -> Git -> Terminal -> Problems -> Editor`.
  - Focused surface exhibits a distinct visual focus border or indicator.
  - All keyboard input routes exclusively to the currently focused surface.
  - `Escape` from any surface untraps and immediately returns focus to the active Editor view.
  - Non-existent or hidden surfaces are skipped in the cycle cleanly.
- Verify:
  - Unit tests for `lux-ui::focus::FocusRing`.
  - Verification of F6 cycle and Escape untrap across all visible panels.

### G20 — Complete AccessKit Tree & Real Screen-Reader Validation
- Status: `OPEN`
- Depends: G06, G07, G12, G19
- Goal: Ensure accessibility trees cover all interactive surfaces and validate against real platform screen readers.
- Done when:
  - Full AccessKit node hierarchy covers editor text lines, cursors, selection, tabs, status bar, file tree items, search results, and terminal output.
  - Screen reader announcements fire on cursor movement, text changes, tab switches, and search match updates.
  - Real hardware validation performed:
    - macOS: Apple VoiceOver.
    - Windows: NVDA.
    - Linux: Orca.
  - Results and findings recorded in `docs/STATUS.md` and dedicated receipt.
- Verify:
  - Accessibility tree inspection tests.
  - Real screen-reader navigation pass.

### G21 — Localization & Internationalization (`lux-ui::i18n`)
- Status: `OPEN`
- Depends: G07, G08
- Goal: Wire complete Spanish and English string catalogs to all user-facing UI labels, menus, and status messages.
- Done when:
  - All hardcoded UI strings are replaced with catalog lookups from `lux-ui::i18n`.
  - Locale switching (English / Spanish) dynamically updates UI text and formats numbers/dates.
  - RTL (Right-to-Left) text direction support in layout helpers (`direction_for`) verified with mixed bidirectional text.
- Verify:
  - Automated tests checking catalog key completeness across English and Spanish.
  - Runtime verification with locale set to Spanish (`es`) and English (`en`).

### G22 — Settings Configuration UI & Schema Validation
- Status: `OPEN`
- Depends: G03, G08
- Goal: Expose settings management via JSONC configuration files and a dedicated Preferences interface.
- Done when:
  - User can edit `settings.json` with schema validation, auto-completion, and inline error markers.
  - Changes to `settings.json` hot-reload instantly via `ConfigWatcher` without needing an application restart (font size, line height, tab size, rulers, themes).
  - Preferences command opens a structured settings panel for browsing and toggling options.
  - Settings importer supports migrating configuration from VS Code / Sublime Text presets.
- Verify:
  - `cargo test -p lux-config`.
  - Hot-reload verification when saving modifications to `settings.json`.

---

## Track 5 — Cross-Platform Desktop Parity (Windows & Linux)

### G23 — Windows Desktop Runtime Acceptance & Parity
- Status: `OPEN`
- Depends: G06, G17
- Goal: Run and certify the full Lux desktop binary on Windows 10/11.
- Done when:
  - Builds cleanly with MSVC target (`x86_64-pc-windows-msvc`).
  - Font discovery loads Windows monospace fonts (`Consolas`, `Cascadia Code`).
  - GPU rendering operates via Direct3D 12 / Vulkan through `wgpu`.
  - Windows ConPTY terminal functions properly in the integrated terminal panel.
  - Windows desktop IME candidate window and preedit display accurately.
  - High-DPI scaling factors (125%, 150%, 200%) apply sharp glyph rasterization and coordinate transforms.
  - `docs/STATUS.md` Windows columns move from `not-started` to `implemented` for all certified features.
- Verify:
  - Windows CI job and local Windows test run with receipts recorded.

### G24 — Linux Desktop Runtime Acceptance & Parity
- Status: `OPEN`
- Depends: G06, G17
- Goal: Run and certify the full Lux desktop binary on Linux under both X11 and Wayland.
- Done when:
  - Builds cleanly on Linux (`x86_64-unknown-linux-gnu`).
  - Fontconfig probes and finds system monospace fonts (`DejaVu Sans Mono`, `Liberation Mono`).
  - GPU rendering functions via Vulkan and OpenGL fallback.
  - Integrated terminal operates with native Linux PTY.
  - IME input methods (`IBus`, `Fcitx`) work without dropping characters or focus issues.
  - High-DPI scale factors and window decorations behave correctly under GNOME / KDE.
  - `docs/STATUS.md` Linux columns move from `not-started` to `implemented` for all certified features.
- Verify:
  - Linux CI / test environment run with receipts recorded.

---

## Track 6 — Packaging, Distribution & v1.0.0 Release

### G25 — Packaging, Installers & Distribution Bundles
- Status: `OPEN` / partially `BLOCKED` on code signing certificates
- Depends: G23, G24
- Goal: Generate production distribution packages for all desktop operating systems.
- Done when:
  - macOS: Universal application bundle (`lux.app` containing `aarch64` and `x86_64`), codesigned, notarized with Apple Developer certificate, and packaged as `.dmg`.
  - Windows: Signed `.msi` or `.exe` installer (WiX / NSIS) and validated `winget` submission manifest (`packaging/winget/Lux.Lux.yaml`).
  - Linux: Package scripts generating `.deb`, `.rpm`, standalone `AppImage`, `Flatpak`, and Homebrew tap formula.
  - Unsigned fallback packages clearly documented in release artifacts when certificates are unavailable.
- Verify:
  - Clean install and first-launch verification from generated package on clean test environments.

### G26 — Local Crash Reporting & Ed25519 Verified Auto-Update
- Status: `OPEN`
- Depends: G25
- Goal: Wire `lux-pack` for safe local crash diagnostics and cryptographic auto-updates.
- Done when:
  - Panic hook writes detailed local crash logs to user application support directory (`~/.local/share/lux/crashes`, `~/Library/Application Support/Lux/crashes`, `%APPDATA%\Lux\crashes`).
  - Confirmed zero telemetry or background network upload of crash reports.
  - Auto-update verification client downloads manifest and updates, verifying Ed25519 digital signatures before applying.
  - Release channel switching (Stable / Nightly) functions as expected.
- Verify:
  - `cargo test -p lux-pack`.
  - Tampered update package rejection test.

### G27 — Version Bump, Benchmark Certification & v1.0.0 Release Gate
- Status: `OPEN`
- Depends: G06 through G26
- Goal: Formally certify all v1.0 deliverables, publish benchmark budgets, and tag `v1.0.0`.
- Done when:
  - Workspace `Cargo.toml` version bumped from `0.0.0` to `1.0.0`.
  - Full CI test suite (`cargo run -p xtask`) passes across all desktop targets:
    - `cargo fmt --all --check`
    - `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo test --workspace`
    - `cargo deny check`
    - `./scripts/check-limits.sh` (600 lines / 25 fields)
    - `./scripts/check-core-isolation.sh`
  - All performance budgets in `docs/PERFORMANCE.md` re-benchmarked and recorded on release binaries:
    - Cold startup < 150 ms (macOS) / < 250 ms (Win/Linux)
    - Keypress to pixel latency (p99) < 8 ms
    - 60/120 Hz smooth scrolling with no frame drop
    - 100 MB file open < 1 s with RSS < 3x file size
    - Binary size < 40 MB compressed
  - `README.md` and release notes published. Tag `v1.0.0` generated.

---

## Track 7 — Mobile Hosts (Phase 12 iPadOS/iOS & Phase 13 Android)

### G28 — iPadOS & iOS Native Host Implementation (`v1.1.0`)
- Status: `OPEN`
- Depends: G27 (Desktop v1.0 spine complete)
- Goal: Complete the native Swift UIKit host (`host-ios/`) and Rust FFI bridge (`lux-platform::uikit`) for iPadOS/iOS.
- Done when:
  - Fix FFI text transfer: Swift passes UTF-8 bytes and length to Rust; Rust updates buffer and cursor instead of discarding input (`lux_ios_insert_text`).
  - Implement text deletion (`lux_ios_delete_backward`).
  - `MTKView` / `CAMetalLayer` renders editor surface via Metal backend in `lux-render`.
  - Virtual and external hardware keyboard input (`UIKeyCommand`) supports navigation and editing shortcuts.
  - On-screen touch selection handles, caret placement, and loupe magnifier work natively.
  - File access integrates with `UIDocumentPickerViewController` and security-scoped URLs.
  - Multitasking support: Stage Manager, Split View, and dynamic window resizing.
  - Mobile capability profile enforced: Terminal and child process spawning are strictly disabled (ADR 0006).
  - Xcode project builds and runs on iOS Simulator and physical iPad hardware.
- Verify:
  - Execution on iOS Simulator / device.
  - Update `docs/STATUS.md` Phase 12 columns based on device evidence.
- Out of scope: Terminal panel or LSP server process spawning on iOS (permanently excluded).

### G29 — Android Native Host Implementation (`v1.2.0`)
- Status: `OPEN`
- Depends: G28
- Goal: Complete the Kotlin Android host (`host-android/`) and JNI bridge (`lux-platform::android`) for Android 8.0+.
- Done when:
  - Fix JNI FFI boundary: `Java_lux_android_host_LuxActivity_nativeOpenUri` correctly extracts string characters from `jstring` using `GetStringUTFChars` / `ReleaseStringUTFChars`.
  - Implement full JNI methods for text input, backspace, and cursor movement.
  - Custom `InputConnection` handles soft keyboard events, auto-correct, and IME composition candidates.
  - Rendering pipeline connects `SurfaceView` / `ANativeWindow` to Vulkan / OpenGL ES via `wgpu`.
  - Storage Access Framework (SAF `ACTION_OPEN_DOCUMENT`) opens and saves document content URIs through Android content resolver.
  - Touch gestures (tap to place cursor, drag to select, pinch to zoom) operate smoothly.
  - Gradle / NDK build script produces working APK and AAB binaries.
  - Verified on physical Android devices (including ARM Mali and Adreno GPUs).
  - Update `docs/STATUS.md` Phase 13 columns based on device evidence.
- Verify:
  - Android emulator and physical device run receipts.
- Out of scope: Child process shell / terminal on Android (excluded by ADR 0006).

---

## What Not To Do (Binding Non-Goals)

In accordance with `CLAUDE.md` Golden Rule #7 and `docs/NON-GOALS.md`, **never implement or propose** any of the following throughout v1.0, v1.1, or v1.2:
- **No Debugger / DAP:** No breakpoints, step debugging, variable inspection, or call stack frames.
- **No Real-Time Collaboration:** No WebSockets, CRDTs, multi-user presence, or peer-to-peer sync.
- **No Cloud Settings / User Accounts:** No login, remote sync, or proprietary account systems.
- **No Telemetry:** No tracking, metrics gathering, or automated error uploads.
- **No AI / LLM Integrations:** No integrated chat, autocomplete models, or copilot sidecars.
- **No Remote Extension Marketplace:** No remote package repository downloads; only local file/directory plugins.
- **No Web / Browser Build:** No WASM-in-the-browser editor.
- **No Complex Data Tooling:** No notebooks, database viewers, or spreadsheets.
