# Implementation audit

Started 2026-09-08 against the current worktree. This audit is ongoing;
historical receipts and roadmap checkmarks are claims to verify, not proof
of current product behavior. The original roadmap through v1.2 remains the
scope. No phase is newly certified complete by this document.

## Confirmed gaps

| Requirement / claim | Current source evidence | Remaining work |
|---|---|---|
| Usable file editor / Phase 4 file opening | `document.rs` connects argv/native open, save/save-as, new document, dirty history checkpoints, confirmed close through worker notifications, recovery snapshots on edit (`RecoveryStore`), and tab bar tracking | Watcher conflict UI and platform-specific acceptance |
| Core editing available in the application | `input.rs` connects transactional insert/delete, clipboard copy/cut/paste, mouse selection, word/line editing, indent/dedent, language-aware line comments, Shift selection, multicursor movement, IME commit, configurable keymaps (`lux-config`), snippet placeholder navigation (`lux-core`), and Tree-sitter syntax highlighting (`lux-syntax`) | Verify running GUI workflows beyond automated tests |
| Virtualized frame construction | Uses `Buffer::line_content` / `line_start_byte`, intersects selection ranges with viewport, and applies Tree-sitter syntax highlighting to visible lines | Allocation regression test passes for two million lines; republish full frame-time budgets and handle extremely long individual lines |
| Phases 3–7 complete as a product | Desktop binary integrates syntax highlighting (`lux-syntax`), keymaps (`lux-config`), tabs (`lux-ui`), snippets (`lux-core`), and session recovery (`lux-workspace`) | Connect Git, LSP, terminal, and plugin controllers into GUI panels |
| Terminal and task panels | `lux-ui/src/surfaces.rs` counts detected links; App has no PTY/session/task ownership or panel rendering | Connect real terminal lifecycle, rendering, input and tasks |
| Plugin integration | `surfaces.rs` constructs empty grants; App does not install or run plugins | Connect trust, installation, extension dispatch and permission handling |
| Full keyboard panel navigation | App no longer routes F6 to absent panels; editor is the only active surface. Previously F6 changed focus without changing keyboard dispatch | Create real panels and route input to the visible focused target |
| Mobile editing | UIKit Rust insertion parses then discards text; deletion is empty. Swift passes `String` to a C pointer ABI. Android treats a JNI string as a C string and discards it | Implement compatible native bridges, editor state, surface lifecycle and native project builds |
| Mobile acceptance tests | `uikit.rs` and `android.rs` tests assert source substrings | Build and execute native hosts; test real text, IME, document and rendering paths |
| Cross-platform executable startup | `crates/lux/src/font.rs` now searches real macOS, Windows and common Linux system monospace paths and rejects files that do not parse as fonts | Build and run the executable on Windows/Linux; source-level discovery is implemented but not platform acceptance evidence |
| OS-respecting UI scaling | Live Retina acceptance shows visibly undersized text; app metrics remain fixed physical pixels | Apply scale-factor changes consistently to layout, glyph rasterization and input coordinates |
| Atomic save / encoding preservation | Fixed-name temporary file could truncate unrelated data; save dropped modes and replaced symlinks; Unicode decoding and Latin-1 saving could lose data silently | Unique temporary files, atomic backups, basic permissions, symlink preservation and strict encoding checks now have regression coverage; external-write races and full metadata preservation remain separate concerns |
| Packaging safety | `scripts/package.sh` previously recursively removed arbitrary caller-selected destinations before building | Fixed to reject existing destinations, build first and require an executable; full installer work remains |
| Published/released versions | Roadmap says released, while workspace version is `0.0.0` | Inspect release/tag/CI artifacts; source alone does not establish publication |
| Documentation and quality command | README described Phase 0 and advertised an unconfigured `cargo xtask` alias | README now describes the shell and uses `cargo run -p xtask` |

`docs/STATUS.md` already marks many disconnected domain capabilities as
`not-started`. Its chronological scope notes sometimes describe intermediate
states; read them as historical records. Phase-level `done` labels in
`GOALS.md` must not be interpreted as application acceptance.

## Current verification

- `cargo fmt --all --check`: passed on 2026-09-08.
- `bash scripts/check-limits.sh`: passed on 2026-09-08 (heuristic check).
- `cargo test --workspace --locked`: passed on macOS on 2026-09-08 after
  a network-enabled retry downloaded dependencies (exit 0). Real-language-
  server tests are explicitly ignored by default; this does not establish
  live LSP interoperability or application integration. Local Cargo reports
  1.98.0 (Homebrew), whereas `rust-toolchain.toml` requests 1.97.1; the pinned
  toolchain is not certified by this run.
- `bash -n scripts/package.sh` and `git diff --check`: passed.
- `bash scripts/package.sh .`: rejected the existing destination before
  building or deleting anything. Full release packaging is not yet verified.
- After keyboard/render integration: `cargo test -p lux-core -p lux-ui -p lux
  --locked --quiet`: 409 tests passed on macOS. The initial sandboxed run
  failed two existing Finder/trash tests; the unrestricted retry passed all
  tests without skips. This is handler/geometry verification, not a manual
  GUI or physical IME acceptance pass.
- New regression coverage includes reversed Unicode selection undo/redo,
  atomic multicursor replacement and deletion, primary-cursor restoration,
  no-op deletion preserving redo, command chords suppressing text, IME
  confirmation, CRLF insertion, vertical/page movement, mixed line endings,
  selected primary caret geometry, and viewport-bounded frame allocation.
- `cargo clippy -p lux -p lux-ui -p lux-core --all-targets --locked -- -D
  warnings`: passed with local Rust 1.98.0. Updated the UTF-16 decoder's
  fixed-size chunk iteration to the equivalent `as_chunks` API for its new
  Clippy lint. Formatting, file/struct limits and diff whitespace checks pass.

## Desktop integration verification (2026-09-28)

The desktop binary (`crates/lux`) now integrates:
- **Configurable keymaps** (`lux-config::Keymap`): `ChordSequence` matching via `Keymap::command_for`, platform modifier resolution, and command dispatch (copy, cut, paste, undo, redo, select_all, duplicate_line, delete_line, join_lines, indent, dedent, toggle_comment).
- **Snippet expansion & interactive navigation** (`lux-core::snippets`): `App::expand_snippet` expands templates with tabstops and mirror placeholders; `Tab` / `Shift+Tab` cycles between placeholders; editing replaces active placeholder; `Escape` exits snippet mode.
- **Tree-sitter syntax highlighting** (`lux-syntax::Document`): incremental syntax synchronization on document open and buffer edits, rendered using theme token colors from `lux-ui` (`LUX_DARK`) on visible viewport lines.
- **Document recovery** (`lux-workspace::session::RecoveryStore`): unsaved buffer changes save recovery snapshots; `App::new` detects recoverable sessions and surfaces notification; clean save/close clears snapshots.
- **Multi-document tabs** (`lux-ui::tabs::TabBar`): documents maintain tab bar state for opened and closed documents.
- **Automated verification**: all 29 tests in `lux` pass; `cargo run -p xtask` (fmt check, clippy with `-D warnings`, workspace tests, cargo deny, 600-line / 25-field limit checks, core isolation check) passes cleanly.

## Remaining desktop editing checks

Clipboard copy/cut/paste, drag and Shift-click selection, horizontal caret reveal,
structural undo grouping, AltGr text input, live accessibility updates and
SetTextSelection actions, configurable keymap commands, snippet placeholder navigation,
syntax highlighting, and crash recovery snapshots all have automated coverage.
Physical macOS/Windows/Linux native window GUI and IME candidate-window acceptance
passes remain as future manual validation.

## Document integration acceptance (2026-09-08)

- After the document, native-dialog and strict-decoding changes,
  `cargo test --workspace --locked --quiet` passed (exit 0), with 31
  pre-existing real-server LSP tests ignored. `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, formatting, core isolation,
  file/struct limits and diff whitespace checks all passed.

- Native macOS run in a temporary `LuxAudit.app`: Cmd+O opened the requested
  test file, Cmd+A and typing replaced its content, Cmd+S wrote `Verified`;
  disk inspection confirmed the exact bytes and original `.bak` content.
  The first capture after Open was blank, while subsequent keyboard editing
  rendered correctly; first-frame presentation after native dialogs still
  needs a dedicated timing check.
- A second native run verified close-button and Cmd+W confirmation,
  Cancel preserving unsaved text, and Yes opening Save As then saving
  `Unsaved acceptance text` to `saved-on-close.txt` before exiting.
- The initial unparented close-dialog run became inaccessible and the
  process was subsequently absent; no crash report established its cause.
  Parent-window confirmation dialogs were then implemented and the above
  acceptance flow passed. Do not infer a general RFD bug from the first run.
- Headless tests use real files and the real worker/result path, with native
  dialogs replaced only at the test boundary. They cover snapshot versions
  while editing during save, cancel/failure retaining dirty content, external
  changes, binary refusal, large-file read-only mode, and reopening a just-
  saved file without restoring stale text.
- Core tests cover unique temporary names, failed-backup cleanup, Unix mode
  and symlink preservation, backup symlinks, immutable save snapshots,
  unrepresentable Latin-1 refusal, malformed BOM payload refusal and saved
  history IDs across coalescing/undo/redo/branching. Non-UTF-8 backup naming
  is tested as a path transformation: this macOS filesystem rejects creating
  such names (`Illegal byte sequence`), including outside the sandbox.
- `tempfile` is now a normal core dependency for exclusive same-directory
  temporary files; `Buffer::clone` shares the rope for worker snapshots and
  is not used for undo history.

## Completion sequence

1. Establish build/test baseline and inspect test coverage against each
   claimed capability; fix failures without weakening acceptance criteria.
2. Finish the desktop document lifecycle and editing command integration.
3. Integrate syntax/navigation, workspace/search/configuration, Git, LSP,
   terminal and plugins; verify complete workflows in the running app.
4. Verify accessibility, localization, performance and desktop platforms.
5. Finish distribution and native mobile implementations, then verify actual
   installations and device behavior. Signing/store credentials and physical
   device checks require external evidence when reached.

Each remaining roadmap item needs source and behavioral evidence before
closure. This initial inventory does not replace the full acceptance audit.
