# Status

> Phase 0 (2026-11-01). 0.1 and 0.2 hold. Encoding round-trips UTF-8 (with
> and without BOM), Latin-1, Windows-1252, and UTF-16 LE/BE with and without
> a BOM when the no-BOM guess is confident. The status bar names the
> encoding. Save is byte-for-byte unless a conversion is requested.
> `docs/compare/characters.png` (macos-15 compare run 41348580486) shows
> áéíóúüñ, emoji, CJK, and combining marks on Monokai, with `UTF-8` in the
> status bar. The default theme is Monokai, parsed from
> `themes/Monokai.sublime-color-scheme` (MIT, `themes/NOTICE.md`). The
> sidebar, tabs, selectors, and status bar take their colors from that
> theme. Scrolling the file tree clips each row to the rectangle under
> the project header and above the status bar. The header stays put. In `docs/compare/monokai-side-by-side.png` from that same run the
> Lux editor well is `#272822`, the current line is `#2C2D27`, the tab bar
> is `#1E1F1C`, and the status bar is `#414339`, beside Sublime Text on the
> same file. A selection on each of the 14 bundled themes paints at least
> 1.30:1 over that editor (`SELECTION_CONTRAST_MIN`). Monokai's selection
> is opaque `#49483E`. The right-hand status labels are light on that bar. The window
> title is the file name and its parent folder. The column ruler is off
> unless `editor.columnRulers` is set. Indent guides are drawn only where
> a line has leading indentation. The minimap is a darker strip at the
> right edge and does not paint a stroke across the buffer. Word wrap
> defaults to off (`editor.wordWrap` in the shipped settings). The View
> menu, the command palette, Settings, and Option+Z toggle it, and the
> choice is written back to that setting. An untitled buffer stays Plain
> Text until a language is chosen or the file is saved with an extension.
> Sublime's own minimap, breadcrumb, and lighter status chrome are
> not copied. Text is a grayscale mask at every display scale, including
> scale 1. The 1× capture used to paint LCD fringes beside `#F8F8F2`.
> The job logged `display scale 1`, so the PNG does not prove Retina.
> Confirming a theme reloads settings. `editor.lineHeight` below 8 is a
> multiplier of the font size (the default 1.4 is not 1.4 pixels). The
> font size and that line height are then multiplied by the window's
> display scale, so a theme change keeps the buffer, its language, and
> its clean/dirty flag, and the text stays on screen. A new buffer is
> untitled, clean, and Plain Text. The palette lists one `Color Theme`
> command. Its text is the system UI font at 13pt times the display
> scale. Row text and match highlights meet WCAG AA against the row
> fill. The selected row starts from the theme's panel-row color and
> has no accent stripe. Settings has a category sidebar (All, Editor,
> Appearance, Keys, Files, Git, CLI), a search field, and a scrollbar.
> Settings and palette text is the system font at 13pt or larger.
> `ten_bundled_themes_paint_ui_text_at_wcag_aa` computes text contrast
> on the ten bundled themes from Monokai through Catppuccin Latte.
> `ten_bundled_themes_paint_controls_at_wcag_aa` requires the toggle
> track and the selected category to clear 3:1, the text on them to
> clear 4.5:1, and a selected stepper's value and its −/+ chips to
> clear 4.5:1 and 3:1 against that selection, on those same ten themes. `scripts/settings-e2e.sh` opens Settings, scrolls to the end,
> cycles those ten themes with the buffer and cursor in place, and
> checks the theme after a restart. A right click or Control-click in
> the buffer opens a context menu: Cut, Copy, Paste, Select All, Toggle
> Comment, Go to Definition when that command is registered, Stage Hunk
> and Unstage Hunk when those commands are registered, then Command
> Palette. Cut and Copy are disabled with no selection. The git rows are
> disabled when the caret is not on a diff hunk. There is no Revert Hunk
> command, so the menu does not add one. A right click outside the
> selection moves the caret there; a right click inside it keeps the
> selection. The menu's top-left corner is the pointer, and it flips
> inside the window only when it would not fit. The panel and every row
> share one logical layout scaled into physical pixels, and the panel is
> as wide as the longest label plus its padding. A double click selects
> the word under the pointer, and dragging from that click extends the
> selection by whole words. A third click on the same line selects that
> whole line, and dragging from it extends the selection by lines.
> The free macos-15 runner stays 1× unless a job records a scaled mode,
> so a Retina capture of an external monitor is still solo-Mac.
> `scripts/theme-break.sh` opens a Rust file on every bundled theme,
> checks the editor, minimap, tabs, status bar, palette, and settings
> by pixel, and writes `docs/evidence/themes/`.
> The language picker uses that same card, ranks names with the palette's
> fuzzy matcher, scrolls its own list, and applies the language on Enter
> or a row click. Shift+Command+M and the status-bar language label open
> it. A chained typing undo groups one word; a one-second gap before the
> next edit starts another step, and undo restores the earlier selections.
> `docs/compare/selectors.png` (compare run 41357599959) shows the command
> palette in Lux, Sublime Text, and Zed with the query `sel`. Lux's card
> sits under the tab bar, about half the window, with a shadow. The editor
> stays visible above and beside it. The field shows `sel`. The selected
> row uses the theme's panel-row color, and matched letters stay at or
> above WCAG AA against that row. Go to file keeps a contiguous run or a word-initial match
> and drops a scattered hit when one of those exists. `mon` on the compare
> fixtures keeps `monokai.rs`. The `sample-*.bin` names match only as
> `m`…`o`…`n` (`sample`, a later `o`, and the `n` in `.bin`).
> `docs/compare/` is compare run 41363899505. `goto.png` is Lux, Sublime,
> and Zed with `mon`; each list is `monokai.rs`. `selectors.png` is the
> palette with `sel`, which Forge accepted. `theme.png` is the theme
> picker with `mo`. `retina-palette.png` is that Lux frame from the
> renderer at scale factor 2 (2000×1312). The runner logged display scale
> 1, so Sublime is not in that file. Forge accepts that framebuffer as
> Lux's Retina render and notes that the runner cannot capture Sublime
> in HiDPI. Selector open motion is 120 ms: the card fades in and slides
> 8 px down under the tab bar. Reduced motion skips it. The compare PNG
> is the settled frame.
> `overlay_motion::the_open_animation_fades_and_slides_across_three_frames`
> samples the start, the middle, and the end. Block 0 is closed.
> Phase 1 file tree and tabs are hecho. Compare run
> [41394350609](https://github.com/lcajigasm/lux/actions/runs/41394350609)
> (`docs/compare/tree.png`, `docs/compare/tabs.png`) shows Lux, Sublime
> Text, and Zed on the fixtures folder, and the same three with
> `monokai.rs` and `characters.txt` open. Lux's inactive tab title is
> the foreground. The runner logged display scale 1. Block 1 stays
> parcial: the status bar labels and clicks are in ST-FIX-12. External reload, hot exit, and Command+/ are in ST-FIX-13. Minimap and splits stay parked. Settings (Command+,) is an opaque page between the tab bar and the status bar. Each row is a label, a description, and a control: a toggle, a stepper, or a menu. The page edge is the neutral chrome border. Selected text stays the foreground. The selection is inset inside the page. `docs/compare/settings.png` is that page from the renderer at scale factor 2.
> Workspace version 1.4.0 is the block 0 release. Workspace version
> 1.5.0 is the file-tree and tabs release. See [ROADMAP.md](ROADMAP.md).
>
> Current gap matrix (2026-10-31): [ROADMAP.md](ROADMAP.md). Historical audit: [AUDIT.md](AUDIT.md).
> v1.3.1: macOS is the product focus. Linux and Windows are secondary.
> Pull-request and main CI keep the `ubuntu-latest` jobs and add one standard
> `macos-15` job: `cargo test --workspace`, then a universal `.app` / `.dmg`
> uploaded as a CI artifact and not published. The debug tree is removed
> before that image, and the release trees plus the cargo registry are
> removed after the fat binary is in the bundle. The disk image is a
> writable volume whose size is the staged tree plus margin, then
> converted to UDZO, after any leftover `/Volumes/Lux` is detached.
> `hdiutil create -srcfolder` filled the disk on runs 41335548609 and
> 41341898224. Windows is not in CI.
> A `vX.Y.Z` tag still attaches `lux-linux-x86_64` and `lux-linux-amd64.deb`
> from the Linux workflow. The deb depends on `libfontconfig1` and `libegl1`
> (the EGL GPU loader named in the release binary; `libvulkan.so.1` is not in
> that binary), plus the xkbcommon, X11, and Wayland libraries it dlopens
> before a window exists.
> `release-macos.yml` is a separate one-job standard `macos-15` workflow
> (`timeout-minutes: 45`, cargo cache, universal ad-hoc `.dmg` via
> `scripts/package-macos.sh`). After the upload it appends the `.dmg`
> line to the release `SHA256SUMS`. It starts when that GitHub Release is published,
> or on `workflow_dispatch`. It does not run on pull request, branch push, or
> tag push, so the Linux workflow does not start a second macOS job.
> The bundle also contains `Contents/SharedSupport/bin/lux`. That command
> returns after handing its paths to the running editor, or it starts
> `Contents/MacOS/lux` when none is running. A second `lux file` on Unix
> does the same handoff. `--probe`, `--snapshot`, and `--bench-open` stay
> in the process that received them.
> The repository is public. These jobs use standard runners only.
> Windows `.msi` stays `scripts/package-msi.ps1` (unsigned WiX).
> `pages.yml` deploys `site/` on push to `main` and on `workflow_dispatch`.
> Editor pictures on the site and in the README come only from
> `scripts/screenshots.sh` (standard `macos-15`, `workflow_dispatch` or a
> published release). That job builds `lux` and captures its window.
> Appearance versus Sublime, VS Code, and Zed is [macos-parity.md](macos-parity.md).
>
> The Linux CI test job runs `headless_session_opens_types_saves_palette_split_and_find`
> (`crates/lux/src/desktop_pass.rs`). It drives `handle_keyboard_event`: open a file,
> type, save, command palette (`Ctrl/Cmd+Shift+P`), split (`Ctrl+Alt+2`), and find.
> The same test builds the AccessKit tree and checks `Window`, `MultilineTextInput`,
> `TextRun` (the buffer text), `Search`, and `TextInput` (the find query). It does
> not open a GPU surface. What stays manual: pixels on screen, the IME candidate
> window, and the native file dialog. This environment has no DRM render node, so
> a real window was not opened here. VoiceOver, NVDA, and Orca were not run.
>
> The 8 ms typing budget is asserted in release mode by
> `test_continuous_typing_latency_large_file_under_8ms` (CI job `release-latency`
> on `ubuntu-latest` only). It times `insert_at_cursor` plus syntax sync on a
> 20,000-line buffer. It is not keypress-to-pixel. On this Linux x86_64 machine,
> 2026-10-31, release mode measured p99 and max at 376 µs (30 inserts; the test's
> p99 slot is the slowest sample). A hosted run the same day, before CI was cut
> to Linux, measured 215 µs on `ubuntu-latest` and 1.06 ms on `windows-latest`.
> Debug CI keeps a 120 ms ceiling. macos-15 debug run 41357599950 measured
> 82 ms on that test. The 8 ms release assert is not widened.
>
> v1.1.1: `editor.insertFinalNewline` defaults off (Sublime / VS Code). A save the worker
> rejects or cancels undoes that transform when the user has not typed since. Ctrl+Alt+digit
> layout chords work on Windows and Linux without swallowing AltGr text. The plugin host
> is wasmtime 48.0.8 (RUSTSEC-2026-0422, 0423, 0424). A `vX.Y.Z` tag builds
> the Linux packages in `.github/workflows/release.yml`. The universal macOS
> disk image is `.github/workflows/release-macos.yml` and is not part of pull-request CI.
>
> Current audit (2026-10-10): see [AUDIT.md](AUDIT.md). The desktop binary
> is certified for **v1.0.0** release: integrates transactional editing, configurable keymaps,
> snippet expansion, tree-sitter syntax highlighting, multi-document tab strip, interactive status bar,
> unified command palette (`Cmd+Shift+P`), Goto Anything (`Cmd+P`), in-buffer find/replace (`Cmd+F`/`Cmd+H`),
> workspace file tree sidebar (`Cmd+B`), minimap overview with click-to-scroll, Sublime Option+Click multi-cursor
> placement, in-process Git integration (`gix`), non-blocking LSP client (`async-lsp`), integrated PTY terminal panel
> (`portable-pty`), sandboxed WASM plugin host (`wasmtime`), F6 focus ring navigation, complete AccessKit screen-reader
> tree with live announcer (`ANNOUNCER_ID`), full Spanish and English i18n, layered settings UI & JSONC hot-reloading with
> config importers, zero-telemetry local crash reporting (`lux-pack`), and release packaging (`scripts/package.sh`).
> Full workspace tests, limits checks, benchmarks, and xtask verification pass cleanly.

Opening a file asks `lux-git` on a background thread for the branch and
the gutter hunks of that buffer. `Git: Refresh Status`, `View File Diff`,
`Stage Current File`, `Stage Hunk`, `Unstage Hunk`, `Commit Staged Changes`,
`Blame`, and `File History` run there too. `Unstage Hunk` puts that
staged hunk's `HEAD` lines back into the index and leaves the file on
disk. The status-bar branch, `Git: Switch Branch`,
and `Git: Create Branch` open the branch list. A checkout runs only when
the worktree is clean. Creating a branch points `HEAD` at the current
commit and leaves the files alone. `Git: Refresh Status` draws `M`, `A`,
or `D` on the changed row in the file tree, and the same letter on each
parent directory. Conflict markers in the buffer still resolve in the
editor. `Git: Accept Current Change`, `Git: Accept Incoming Change`, and
`Git: Accept Both Changes` resolve the block under the caret and keep that
text on the tab. See `docs/GIT.md`.

Quitting writes the window's position, size, fullscreen or maximized state,
and monitor into the session (`lux_workspace::WindowFrame`). The next launch
creates the window there. If that monitor is no longer connected,
`restore_window` moves it onto a visible one. The same quit writes open
tabs, cursor offsets, and unsaved buffer text; a launch with no path
restores them. The editor has one window, stored as the single entry of
`Session.windows`.

`lux file:line:col` opens that file and places the cursor. `lux -` reads
stdin into an untitled buffer. `--wait` blocks the client until those
buffers close. `--new-window` starts another window. The palette commands
`Shell Command: Install 'lux' in PATH` and `Shell Command: Uninstall 'lux' from PATH`,
and the Tools menu items `Install Command Line Tool` and
`Uninstall Command Line Tool`, link `Contents/SharedSupport/bin/lux`.
`packaging/homebrew/lux.rb` is the cask with that `binary` stanza. The live
tap stays on the published 1.5.6 dmg, which does not contain that binary yet.

Current desktop integration (updated 2026-10-10): the native application
integrates configurable keymap command dispatch (`lux-config`), snippet expansion
with interactive placeholder navigation and tab stops (`lux-core`), tree-sitter
syntax highlighting for opened buffers with Lux Dark token colors (`lux-syntax`),
multi-document tab tracking (`lux-ui`), document recovery snapshots on edits
with crash recovery detection on startup (`lux-workspace`), bottom interactive status bar
with real-time telemetry and mouse click toggles (`crates/lux/src/status_bar.rs`),
unified floating command palette (`Cmd+Shift+P`) with fuzzy search and execution (`crates/lux/src/palette.rs`),
Goto Anything (`Cmd+P`) with `:line` jump navigation (`crates/lux/src/goto.rs`),
docked in-buffer Find and Replace bar (`Cmd+F` / `Cmd+H`) with match highlights and transactional edits (`crates/lux/src/search.rs`),
workspace folder tree sidebar (`Cmd+B` / `Cmd+K, Cmd+B`) with lazy node expansion and tab activation (`crates/lux/src/sidebar.rs`),
right-edge minimap overview (`crates/lux/src/minimap.rs`) with line density bars, viewport indicator, and click-to-scroll navigation,
in-process Git integration (`crates/lux/src/git.rs`) with gutter change indicators, floating hunk diff cards, file staging, and commit execution,
non-blocking LSP client (`crates/lux/src/lsp.rs`) with diagnostics squiggles, floating autocompletion popup, hover documentation tooltip, and problems dock,
integrated PTY terminal panel (`crates/lux/src/term.rs`) with color VT grid, named shells, background task panel, and compiler error link detection,
sandboxed WASM plugins host (`crates/lux/src/plugin.rs`) with fuel caps and capability permission dialogs,
keyboard navigation ring (`crates/lux/src/focus.rs`) with F6/Shift+F6 cycling and Escape untrap,
AccessKit accessibility tree (`crates/lux/src/accessibility.rs`) with live status announcer,
100% Spanish/English catalog coverage (`lux-ui::i18n`),
and Preferences modal UI (`crates/lux/src/settings.rs`) with JSONC hot-reloading and settings migration importers,
smart tabstop indentation and Enter auto-indent with bracket split (`crates/lux/src/input/indent.rs`),
dynamic Language Mode picker modal & status bar switch (`crates/lux/src/language_picker.rs`),
macOS native menu bar integration (`crates/lux/src/menu/macos.rs`) with live view checkmarks,
gutter and line numbers rendering with active line background highlight strip (`crates/lux/src/render/text.rs`),
robust CLI flags dispatch (`--help`, `-h`, `--version`, `-v`) avoiding false document opens (`crates/lux/src/main.rs`),
transient overlay dismissal on editor canvas interaction (`crates/lux/src/app.rs`),
horizontal and vertical split editor layout (`crates/lux/src/split.rs`, `crates/lux/src/render/split.rs`) with real-time buffer synchronization,
Tab Strip 2.0 (`crates/lux/src/document/tab_ops.rs`) with drag-and-drop reordering, pinned tabs, and context menu actions,
soft word wrap toggle (`crates/lux-render/src/text_layout.rs`, `crates/lux/src/app/views.rs`) via Option+Z,
project search with file include/exclude filters (`crates/lux/src/project_search.rs`),
and diagnostic navigation with `F8`/`Shift+F8` (`crates/lux/src/lsp.rs`),
and Sublime multi-cursor occurrence, column, scope & line manipulation suite (`crates/lux/src/app/selection.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux-core/src/selection_ops/scope.rs`, `crates/lux-core/src/edit_ops/lines.rs`): Quick Add Next (`Cmd+D`), two-chord Skip Occurrence (`Cmd+K, Cmd+D`), Select All Occurrences (`Ctrl+Cmd+G` / `Alt+F3`), Split Selection into Lines (`Cmd+Shift+L`), Column/Box selection via Option+Drag, Add Cursor Above/Below (`Cmd+Option+Up/Down`), parallel multi-cursor paste, Expand Selection to Line (`Cmd+L`), Expand Selection to Scope (`Cmd+Shift+Space`), Expand Selection to Tag (`Cmd+Shift+A`), Swap Line Up/Down (`Cmd+Ctrl+Up/Down`), Duplicate Line/Selection (`Cmd+Shift+D`), and Join Lines (`Cmd+J`),
and Sublime code folding system (`crates/lux-core/src/folding.rs`, `crates/lux/src/app/folding.rs`, `crates/lux/src/render/text.rs`): gutter fold triangles (`▼`/`▶`), inline folded block pill marker (`⋯`), keyboard folding (`Cmd+Option+[` / `Cmd+Option+]`), Fold All (`Cmd+K, Cmd+1`), Unfold All (`Cmd+K, Cmd+J`), and Fold by Level (`Cmd+K, Cmd+2..5`),
and Sublime auto-pairing, auto-closing & bracket matching highlight suite (`crates/lux/src/input/autopair.rs`, `crates/lux-ui/src/editor_view/mod.rs`, `crates/lux/src/render/text.rs`): auto-close delimiters `()`, `[]`, `{}`, `""`, `''`, `` ` ` ``, selection wrapping, smart step-over, smart backspace for empty pairs, alphanumeric apostrophe guard, real-time matching delimiter highlights, and `Control+M` jump to matching bracket,
and Sublime Goto Anything integral suite (`crates/lux/src/goto/mod.rs`, `crates/lux/src/goto/search.rs`, `crates/lux-syntax/src/symbols.rs`, `crates/lux-ui/src/goto_anything.rs`, `crates/lux/src/render/goto.rs`): `Cmd+P` with unified syntax for fuzzy files, `:line`, `:line:col`, `@symbol` (functions, structs, classes, traits, enums via Tree-sitter), `#term` document search, composite `file:line`, `file@symbol`, `file#term`, category badges, live interactive preview, and transient snapshot restoration on Escape,
and Sublime Goto Symbol in Project suite (`crates/lux-workspace/src/symbols.rs`, `crates/lux-syntax/src/symbols.rs`, `crates/lux/src/goto/mod.rs`): `Cmd+Shift+R` / `Ctrl+Shift+R` with project-wide background symbol indexing, ranked fuzzy symbol search with exact/prefix/substring scoring, relative path indicators, and instant definition jump,
and Sublime Bookmarks suite (`crates/lux-core/src/bookmarks.rs`, `crates/lux/src/app/bookmarks.rs`, `crates/lux/src/render/text.rs`): Toggle Bookmark (`Cmd+F2` / `Ctrl+F2`), Next Bookmark (`F2`), Previous Bookmark (`Shift+F2`), Clear Bookmarks (`Cmd+Shift+F2`), Select All Bookmarks into simultaneous multi-cursors (`Option+F2`), gutter cyan ribbon markers, and mouse click toggles,
and Sublime docked in-buffer Find & Replace bar (`crates/lux/src/search.rs`, `crates/lux/src/render/search.rs`, `crates/lux/src/render/minimap.rs`, `crates/lux-ui/src/search_bar.rs`): `Cmd+F` / `Cmd+Option+F` / `Cmd+H` with live match counter ("X of Y matches"), interactive toggles for Regex, Case Sensitive, Whole Word, In Selection, dual-row Replace with Find / Find All / Replace / Replace All, Alt+Enter multi-cursor expansion, and amber/gold overview ticks on the minimap,
and Sublime Find in Files with virtual "Find Results" buffer (`crates/lux/src/find_results.rs`, `crates/lux/src/project_search.rs`, `crates/lux/src/render/project_search.rs`): `Cmd+Shift+F` with `Find:`, `Where:` (supporting `-glob` and `!glob` excludes), `Replace:`, virtual editable buffer tab in Sublime format (`Searching X files for "query"`, `Y matches across Z files`), double click / Enter navigation to file & line, and `F4` / `Shift+F4` sequential match hopping,
and Sublime Multi-Column, Multi-Row & 2x2 Grid Layouts suite (`crates/lux/src/pane_view.rs`, `crates/lux/src/split.rs`, `crates/lux/src/render/split.rs`, `crates/lux/src/chrome.rs`, `crates/lux/src/input/layout_tests.rs`): real multi-pane architecture via bounded `PaneView` instances with independent viewports, active tabs, multicursors, fold states, bookmarks, and cached Tree-sitter syntax highlighting, standard presets Single (`Cmd+Alt+1`), 2 Columns (`Cmd+Alt+2`), 3 Columns (`Cmd+Alt+3`), 4 Columns (`Cmd+Alt+4`), 2 Rows (`Cmd+Alt+Shift+2`), 3 Rows (`Cmd+Alt+Shift+3`), Grid 2x2 (`Cmd+Alt+5`), direct group focusing (`Ctrl+1..4`), tab group relocation (`Ctrl+Shift+1..4`), click-to-focus on any pane, interactive mouse-draggable split dividers with ratio clamping, strict geometric bounding preventing cross-pane quad or glyph overflow, and full macOS View menu & Command Palette bindings,
and Sublime Build System Runner suite (`crates/lux/src/build.rs`, `crates/lux/src/render/build.rs`, `crates/lux/src/input/build_tests.rs`): Build Runner (`Cmd+B` / `Ctrl+B`) with auto-detection for Cargo, npm, Python, Go, Make, Bash and Rustc, Build With... (`Cmd+Shift+B` / `Ctrl+Shift+B`) fuzzy modal variant picker with live filter query and command preview, background streaming PTY output dock, Cancel Build (`Cmd+Alt+C` / `Ctrl+Break`), and Compiler Error Navigation (`F4` / `Shift+F4`) with interactive jumping to error file, line and column,
and Sublime Project files suite (`crates/lux-workspace/src/project/mod.rs`, `crates/lux/src/project.rs`, `crates/lux/src/render/project_picker.rs`, `crates/lux/src/input/project_tests.rs`): `.sublime-project` and `.lux-project` support with `folders` (path, excludes, name), project-scoped `settings` (tab size, indent), custom `build_systems` fed directly into Build Runner, Quick Switch Project (`Cmd+Ctrl+P` / `Ctrl+Alt+P`) modal picker with query filtering, active project indicators, Open Project, Save Project As, Close Project, and automatic workspace discovery,
and Sublime Iconic Color Themes suite (`crates/lux-ui/src/theme.rs`, `crates/lux-syntax/src/theme/mod.rs`, `crates/lux-syntax/src/theme/presets.rs`, `crates/lux/src/theme_picker.rs`, `crates/lux/src/render/theme_picker.rs`, `crates/lux/src/input/theme_tests.rs`): built-in calibrated themes for Monokai (`#272822`), Mariana (`#303841`), Breakers (`#0C1014`), and Celeste (`#F8F9FA`) with simultaneous UI chrome & Tree-sitter scope token updates, live interactive preview theme picker modal (`Color Theme`), real-time preview swatches, settings persistence, and native macOS App menu integration,
and Sublime Distraction Free Mode suite (`crates/lux-platform/src/window.rs`, `crates/lux-platform/src/desktop/window.rs`, `crates/lux/src/distraction_free.rs`, `crates/lux/src/app/views.rs`, `crates/lux/src/render.rs`, `crates/lux/src/input.rs`, `crates/lux/src/input/distraction_free_tests.rs`): toggle via `Shift+F11` (Win/Linux) and `Cmd+Ctrl+Shift+F` (macOS), borderless fullscreen, complete chrome suppression (tab bar, status bar, sidebar, minimap, line number gutter), dynamically centered fixed-width document column (`editor.distractionFreeColumnWidth`, default 80 characters), mouse hit-testing offset with `text_left`, Escape dismissal back to normal mode, Command Palette command (`view.toggle_distraction_free`), and native macOS View menu integration (`Enter/Exit Distraction Free Mode`),
and Sublime Clean Whitespace on Save suite (`crates/lux-core/src/edit_ops/whitespace.rs`, `crates/lux/src/document/save_ops.rs`, `crates/lux/src/document/whitespace_tests.rs`): automatic trimming of trailing whitespace on save (`editor.trimTrailingWhitespace`), configurable protection for lines with active carets (`editor.trimTrailingWhitespacePreserveCursor`), automatic final newline enforcement at EOF (`editor.insertFinalNewline`), manual Command Palette triggers (`File: Trim Trailing Whitespace`, `File: Ensure Final Newline`), and single-transaction undo/redo integration,
and Sublime Extensible Snippet Engine suite (`crates/lux/src/snippets/`, `crates/lux-core/src/snippets/`, `crates/lux/src/render/snippet_picker.rs`, `crates/lux/src/input/snippet_tests.rs`): Sublime XML (`.sublime-snippet` with CDATA, `<tabTrigger>`, `<scope>`, `<description>`) and JSON formats, recursive custom snippet loading from `~/.config/lux/snippets/`, `~/.lux/snippets/`, and workspace folders, built-in library for Rust, Python, JavaScript/TypeScript, HTML, Go, C/C++, Markdown, CSS, and Lorem Ipsum, contextual scope matching with alias normalization, trigger word auto-expansion on `Tab` with no active selection, interactive multi-tabstop forward/backward traversal (`Tab` / `Shift+Tab`), multi-occurrence mirror synchronization during live typing and backspace, Escape session cancellation, and modal snippet picker (`Snippets: Insert Snippet...` / `Cmd+Alt+J` / `Ctrl+Alt+J`),
and Sublime Keyboard Macro suite (`crates/lux/src/macros/`, `crates/lux/src/render/status_bar.rs`, `crates/lux/src/input/macro_tests.rs`): Sublime Text `.sublime-macro` JSON array format, macro recording toggle (`Ctrl+Q`), live coral `● Recording Macro` status bar indicator, non-recursive real-time event recording for typing, newlines, backspace, delete, word deletions, and cursor motions, instant playback (`Cmd+Shift+Q` / `Ctrl+Shift+Q`) replicated across all active multi-cursors, and persistence to `~/.lux/macros/` and workspace directories,
and Sublime Cloned Views across Split Panes suite (`crates/lux/src/split/clone_view.rs`, `crates/lux/src/split/clone_view_tests.rs`, `crates/lux/src/split.rs`, `crates/lux/src/document.rs`, `crates/lux/src/render/split.rs`, `crates/lux/src/render/window.rs`, `crates/lux/src/palette/registry.rs`, `crates/lux/src/menu/macos_builder.rs`): multiple panels opening the same underlying `Buffer` with live instantaneous synchronization of buffer edits and atomic undo/redo history, independent viewports, scroll positions (`scroll_x`, `scroll_y`), multi-cursors, and selections per cloned pane, automatic cursor shifting and clamping on buffer modifications and undo/redo, mouse-wheel hit testing for independent inactive pane scrolling without moving active focus, `File: Clone View to Group` command opening 2 columns if in single layout and focusing the cloned pane, `File: New View into File` creating a separate tab sharing the unique `buffer_id`, and full Command Palette, keyboard dispatch, and native macOS File menu integration.
The File menu opens a persisted recent-file list (`Open Recent`, ten paths in `recent_files.json` under the Lux config directory, rewritten off the UI thread, with `Clear Recent`), a second process via `New Window` (Shift-Command-N, `--new-window` with a fresh socket), `Save All` (Option-Command-S), and `Reopen Closed Tab` (Shift-Command-T).

Frame construction accesses only visible buffer lines with syntax token ranges clipped to the viewport.
All 212 integration tests pass in `crates/lux` (221 total tests), and `cargo run -p xtask` passes all CI
checks (fmt, clippy with `-D warnings`, workspace tests, cargo deny, file/struct limits,
and core isolation).

Document integration (2026-09-28, ADR 0017): the binary accepts one file path
or starts untitled. Cmd/Ctrl+O/S/Shift+S/N/W connect native dialogs, worker
open/save, new document and confirmed close. A native event wakes completion;
no recurring polling or disk I/O runs on the UI thread. Save completion marks
only its captured history version clean. Existing external content changes
abort a save. Large files open read-only and binary/malformed Unicode input
is refused. Recovery store persists snapshots on dirty edits and cleans up
on clean save or close. Tab bar state tracks active and closed tabs.
Atomic saving uses exclusive temporary files, preserves permissions and symlinks,
and refuses unrepresentable Latin-1 edits.

Source of truth for what actually works, per platform (golden rule #9).
Updated in the same PR that changes behavior. The README promises nothing
this file does not confirm.

Legend: `not-started` · `partial` (behind an `experimental.*` flag, listed
under Experimental below) · `implemented` · `excluded` (permanently out of
scope for that platform by design, not a gap to fill — see the linked ADR).

Phase 0 shipped no UI by design (the master prompt is explicit: "if there's a
window at the end of Phase 0, you moved too fast"). Phase 1 shipped a
complete, headless `lux-core` — see below.

## Core editing (Phase 1)

`lux-core` has no platform-specific code (that's the whole point of golden
rule #2); the "implemented" mark below reflects where it's actually been
*run* — the full test suite (235 tests + a proptest + 30 minutes of
`cargo-fuzz`), all on macOS (aarch64-apple-darwin). It's confirmed to
**cross-compile cleanly for `aarch64-apple-ios` and `aarch64-linux-android`**
(`cargo build -p lux-core --target ...`, no linker needed — it's an `rlib`,
no final binary) — Phase 1's specific portability check — but that's a build
check, not a behavioral one, so those columns stay `not-started` until the
suite actually runs there. Windows/Linux are untested for the same reason
(no non-macOS dev environment available yet); the code itself uses only
`std::fs`/`std::path`, nothing OS-specific.

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Buffer load/save (atomic + backup) | implemented | not-started | not-started | not-started | not-started |
| EOL / indent / encoding detection | implemented | not-started | not-started | not-started | not-started |
| Positions/selections (byte/char/column, grapheme-aware) | implemented | not-started | not-started | not-started | not-started |
| Multicursor + multiselection | implemented | not-started | not-started | not-started | not-started |
| Transactional undo/redo | implemented | not-started | not-started | not-started | not-started |
| Edit operations (indent, comment, duplicate, sort, case) | implemented | not-started | not-started | not-started | not-started |
| Movement (word/line/paragraph/bracket-match) | implemented | not-started | not-started | not-started | not-started |
| Sublime-style selections (expand, next occurrence, box) | implemented | not-started | not-started | not-started | not-started |
| Find/replace in buffer | implemented | not-started | not-started | not-started | not-started |
| Snippets with navigable placeholders | implemented | not-started | not-started | not-started | not-started |
| `lux-core-cli` headless test harness | implemented | not-started | not-started | not-started | not-started |

**Closed gap** (2026-08-12, see `docs/PERFORMANCE.md`'s measurement
history): the "open a 100 MB file" performance budget's RSS half (< 3×
file size) measured 3.16× at Phase 1 close, over budget — three
simultaneous copies of the file's bytes alive at once during
`Buffer::open` (the raw `Vec<u8>`, a decoded `String`, and `ropey`'s own
chunk storage). Left open at the time rather than fixed under close-phase
time pressure, revisited in this pass: `Buffer::open` now goes through a
new `encoding::decode_owned`, which reuses the `Vec<u8>` from `fs::read`
as the decoded `String` directly (`String::from_utf8`, zero-copy) on the
common no-BOM-valid-UTF-8 path, cutting the three simultaneous copies to
two. `Buffer::from_bytes(&[u8])` keeps its original borrowing signature
unchanged — the 250-call-site blast radius the original close-phase
report flagged as the reason not to widen it — since only `open`, the
path the budget actually measures, needed the fix. Re-measured the same
way the original number was produced (`/usr/bin/time -l` around
`lux-core-cli` opening a real 100 MB file): **2.16×** (215.6 MB RSS /
100.0 MB file), inside budget.

## Rendering & desktop shell (Phase 2, `v0.1.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Window/input/IME platform trait | implemented | not-started | not-started | not-started | not-started |
| GPU text rendering (glyph atlas, subpixel AA) | implemented | not-started | not-started | not-started | not-started |
| Desktop IME (preedit, cursor area) | implemented | not-started | not-started | n/a | n/a |
| Accessibility tree (`accesskit`) | implemented | not-started | not-started | not-started | not-started |
| Lux Dark theme | implemented | not-started | not-started | not-started | not-started |
| Virtualized editor view | implemented | not-started | not-started | not-started | not-started |

**Scope note (`lux-platform` trait + `winit` backend — this phase's first
deliverable, tracked in `docs/receipts/phase-02-luxplatform-trait-impl-winit.md`)**:
the trait covers window creation, keyboard/mouse/touch input, IME event
plumbing (`Enabled`/`Preedit`/`Commit`/`Disabled`, cursor-area reporting),
clipboard, native file dialogs, DPI/scale-factor, lifecycle
(`resumed`/`suspended` split from `surfaces_available`/`surfaces_lost` per
ADR 0004), and safe-area insets (always zero on desktop — an accurate
report, not a placeholder). Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-platform` (event-conversion unit tests plus a real
clipboard round-trip against the system pasteboard) and a manual run that
opened a live window and logged genuine resize/focus/occlusion/scale-factor/
mouse events. **Not yet exercised**: live IME composition (Japanese input,
Spanish dead-key accents — Phase 2 deliverable 5, blocked on having
something to render preedit text into, i.e. `lux-render`/`lux-ui`), touch
input (no touch hardware on the dev machine), and file dialogs (a native
modal can't be driven from an automated test, and wasn't manually clicked
through this pass either). Windows and Linux stay `not-started`: this
machine has no `rustup`-managed toolchain, so there's no target sysroot to
even `cargo check` against — unlike Phase 1's pure-Rust `lux-core`,
`lux-platform`'s desktop backend pulls in real per-OS system dependencies
(`windows-sys`, `x11rb`, Wayland), so a cross-compile check isn't meaningful
here the way it was there.

**Scope note (`lux-render`'s `wgpu` context/surface/resize — this phase's
second deliverable, tracked in
`docs/receipts/phase-02-luxrender-contexto-wgpu-superficie-resize.md`)**:
no capability row above moves yet — this is infrastructure underneath "GPU
text rendering", not the row itself, since there's still no glyph atlas, no
quad/text pipeline, and nothing a human can see rendered. What exists and is
verified: `GpuContext` (real headless instance/adapter/device/queue,
`cargo test -p lux-render context::`) and `Surface` (real `wgpu::Surface`
built from a live window's raw handles via `lux-platform`, sRGB-preferring
format selection, resize that's a no-op when the size didn't change,
`cargo test -p lux-render surface::`). Verified manually on macOS: a real
window + real surface configured at its actual size/scale/format
(`Bgra8UnormSrgb` at 1600×1200, matching a Retina display), resized live as
the window resized, and `get_current_texture()` called against the live
surface each redraw attempt (correctly reporting `Occluded` for a
non-frontmost window rather than crashing) — a full clear-color present
wasn't confirmed on screen (the manual test window couldn't be reliably
brought to the foreground from this scripted session), but nothing in that
gap is unverified logic: it's `lux-render`'s render-pass code exercising
the real surface right up to the point a human would need to look at the
screen.

**Scope note (glyph atlas — `etagere` + `swash`, 4×1 subpixel — this phase's
next slice of deliverable 2, tracked in
`docs/receipts/phase-02-atlas-glifos-etagere-swash-4x1.md`)**: still no
capability row movement, same reasoning as above — this is the rasterization
and packing layer underneath "GPU text rendering", with no `wgpu::Texture`
upload, no quad/text render pipeline, no `parley`/`fontique` layout
integration, and no `TextRenderingMode` yet, so nothing reaches a screen.
What exists and is verified (`cargo test -p lux-render glyph_atlas::`):
`GlyphAtlas` packs real rasterized glyph bitmaps — via `swash::scale::Render`
with `Format::subpixel_bgra()`, hinting on — into three `etagere`-allocated
planes (`Monochrome`/`Polychrome`/`Subpixel`, matching `swash`'s own
`Content::Mask`/`Color`/`SubpixelMask`), with a `GlyphKey` lookup cache so a
repeated (font, glyph, size, subpixel-phase) request never rasterizes or
packs twice. The 4×1 subpixel design is verified concretely: the same glyph
rasterized at its 4 horizontal phase buckets lands in 4 distinct atlas
rectangles (real `swash` output, real Ahem test-font bytes, not a stub), and
a plane that runs out of room reports `None` rather than corrupting a
neighboring glyph's bitmap. Cache eviction and `wgpu` texture upload are
explicitly out of scope for this slice — later loops.

**Scope note (`parley` + `fontique` integration — this phase's next slice
of deliverable 3, tracked in
`docs/receipts/phase-02-integracion-parley-fontique-layout-ligaduras.md`)**:
still no capability row movement — this produces `Layout`/cluster data, not
pixels; nothing connects it to the glyph atlas or a render pass yet, so
still nothing reaches a screen. What exists and is verified (`cargo test -p
lux-render text_layout::`): `TextLayoutContext` shapes one line of text at
a time via a real `parley::Layout`, with a font registered directly from
bytes (`fontique::Collection::register_fonts`) rather than system
discovery — `fontique`'s `system` feature is off for a real, load-bearing
reason documented in `docs/DEPENDENCIES.md`: it pulls a `fontconfig` native
binding version that conflicts with the one `lux-platform`'s Wayland CSD
support already pins, and both crates share one workspace dependency graph.
System font enumeration (letting a user pick any installed font) is a
tracked gap, not a placeholder — it needs either dropping `crossfont`'s
conflicting pin or a native binding conflict resolution, neither of which
is this deliverable's job to decide. Verified with the same public-domain
Ahem test font the glyph atlas uses: exact uniform per-character advance on
a monospace grid, font-size scaling doubling the advance proportionally,
empty text producing zero clusters, and per-byte-range OpenType feature
overrides (`calt`/`liga`) accepted and applied without dropping the
line-default features outside that range — real per-run control, the thing
the master prompt asks for. **Not yet verified**: an actual visible
ligature collapse (Ahem defines no OpenType features to trigger one; needs
a real programming font, future work once one ships with the editor) and
IME preedit rendering (needs `lux-ui`, which doesn't exist yet).

**Scope note (`lux-ui`'s virtualized editor view — this phase's
deliverable 4, tracked in
`docs/receipts/phase-02-luxui-vista-editor-scroll-virtualizado.md`)**: the
"Virtualized editor view" row above stays `not-started` for the same
reason every other Phase 2 scope note gives — this produces the geometry a
render pass would draw (line layouts, gutter numbers, cursor/selection
rectangles, indent-guide and column-ruler x-positions), not pixels; the
`wgpu` quad/text pipeline that would paint it doesn't exist yet. What's
real and verified (`cargo test -p lux-ui --test golden_editor_view`, a
20-line real `Buffer` through a viewport that only fits 4 lines): only
those 4 lines are ever shaped — virtualization confirmed by construction,
not by inspection; line numbers right-aligned to the document's actual
digit width; a multi-cursor caret that blinks (a pure function of elapsed
time against a configurable interval) and is silently dropped from the
frame when scrolled off-screen rather than drawn somewhere wrong; a
multi-line selection rendered as one rectangle per visible line, with
correct partial-width rectangles on its start and end lines and full-width
rectangles in between; indentation guides derived from each line's real
leading whitespace and `Buffer::indent()`'s detected width; and column
rulers positioned via the same monospace shaping as the text itself, not a
separately-guessed character width. **Real gap found and deliberately
worked around, not silently papered over**: `lux-core::Buffer` has no
public per-line text accessor or `(line, column) -> Position` constructor
(only whole-document `text()` and a char-range `text_range()`); this view
builds its own byte-range line index from `buffer.text()` once per frame
(`editor_view::line_index`, private to `lux-ui`) rather than touching
`lux-core`, which was out of this deliverable's scope. That index build is
`O(document length)` per frame — fine at the sizes exercised here, a real
cost worth fixing with a proper `lux-core` per-line accessor before this
meets a large-file performance budget. **Not yet verified**: any of this
painted to an actual screen (needs the quad/text pipeline above), and
tab-expansion-aware indent guides (this pass counts every leading space or
tab as one column, matching `Position::column`'s current, not-yet-tab-aware
semantics — see the receipt).

**Scope note (`lux-platform`'s desktop IME — this phase's deliverable 5,
tracked in `docs/receipts/phase-02-ime-escritorio-preedit-area-cursor.md`)**:
the "Desktop IME (preedit, cursor area)" row above stays `not-started`
(macOS included) rather than moving to `implemented`, for two honest
reasons: only macOS was exercised (no Windows/Linux dev environment, same
gap `lux-platform`'s own entregable 1 already documented), and "preedit
visible en su sitio" in the literal sense the master prompt means — inline,
in the actual text position inside the editor — needs the `wgpu` text
pipeline, which doesn't exist yet; this pass showed live preedit text via
the window's title bar instead, an honest stand-in, not the real thing.
What's genuinely new and human-verified here, not just unit-tested:
`ImeComposition` (`lux-platform::ime`), a composition-state machine turning
`winit`'s raw `Enabled`/`Preedit`/`Commit`/`Disabled` sequence into
"here's the text being composed and its cursor" / "here's the final text,"
confirmed against **real macOS IME output**, not just assumptions — a
Spanish accent dead-key (´ then e) produced the exact sequence `Enabled` →
`Preedit("´", cursor_range=(2,2))` → `Preedit("", None)` (winit's own
synthetic clear) → `Commit("é")`, live, in a real window
(`cargo run --example ime_smoke -p lux-platform`), with the window title
updating to show the live preedit and the final commit. That live output
also independently confirmed something the automated tests could only
infer from `winit`'s docs: `cursor_range` is a **byte** offset, not a char
count — `(2, 2)` is exactly after "´", which is 2 bytes in UTF-8. **Not yet
verified**: any CJK IME (no Japanese/Chinese input source was available to
test with), and whether the OS actually anchors its own candidate window
at the position `set_ime_cursor_area` reports — a single dead-key accent
has no candidate list to show, so there was nothing positioned to observe;
that needs a CJK IME session, still open for a future pass on a machine
that has one.

**Scope note (`accesskit` conectado — this phase's deliverable 6, tracked
in `docs/receipts/phase-02-accesskit-conectado.md`)**: the "Accessibility
tree (`accesskit`)" row above stays `not-started` even though this pass's
manual verification genuinely succeeded end-to-end with a real screen
reader — the row tracks this wired into the actual, unified `lux` editor,
which doesn't exist yet (no `crates/lux` binary assembling
`lux-platform`+`lux-render`+`lux-ui` together), and ADR 0005's own
acceptance bar is VoiceOver **and** NVDA **and** Orca, of which only
VoiceOver could be exercised on this machine. What's real: `lux_ui::
accessibility::build_tree` (`cargo test -p lux-ui accessibility::`, 8
tests) builds an actual `accesskit` tree — a `MultilineTextInput` root with
one `TextRun` child per line, real per-character byte lengths/positions/
advances from `lux_render`'s own shaping, and a `TextSelection` tracking
the real primary cursor — confirmed against a **real accesskit_winit
adapter and real VoiceOver** (`cargo run --example accessibility_smoke -p
lux-ui`, project owner's own verification, verbatim: "Todo funcionó como
debería"): VoiceOver announced the control as a multi-line text field,
read its three real lines of content correctly, and tracked the caret
moving in response to real Left/Right arrow input backed by
`lux_core::Position::next_grapheme`/`prev_grapheme`. **Not yet verified**:
NVDA and Orca (no Windows/Linux environment available, same gap every
other Phase 2 deliverable this session has hit), and multi-cursor exposure
(only the primary cursor/selection is exposed — no accessibility API models
more than one caret, so the others are intentionally not represented, not
lost from the editor's own state).

**Scope note (Lux Dark theme — this phase's deliverable 7, tracked in
`docs/receipts/phase-02-tema-lux-dark.md`)**: the "Lux Dark theme" row
above stays `not-started` — this defines the color identity, not paint on
screen; there is still no quad/text render pipeline to apply it to (same
gap every other Phase 2 deliverable this session has hit). What's real
(`cargo test -p lux-ui theme::`, 7 tests): `lux_ui::theme::Theme::LUX_DARK`
carries v0's identity forward verbatim — indigo-black background
`#12121B`, golden-amber accent `#F0B442` — verified by parsing those exact
hex strings from `LUX_PROMPT_MAESTRO.md` and asserting they equal the
constant's fields byte-for-byte, not just eyeballed. `foreground`,
`selection`, and `muted` are this deliverable's own reasonable completions
(every theme needs a foreground; a translucent selection derived from
`accent`), not values pulled from any spec — flagged as such in the code,
not silently presented as part of the documented identity. "Golden capture"
here means a deterministic RGBA8 pixel buffer (`capture_swatch`) rendered
from the theme's real color values via flat fills (no antialiasing, so
exact byte comparison is the correct tolerance — zero), checked pixel-by-
pixel against the expected color at each swatch position. A full
token/scope-based theme system (Lux Light, high-contrast, per-syntax
tokens) is Phase 3's job, not this one's.

**Scope note (performance budgets — cold start, keypress latency, scroll —
tracked in
`docs/receipts/phase-02-presupuestos-rendimiento-arranque-latencia-scroll.md`)**:
none of Phase 2's three named budgets is fully measurable yet, for the
same reason as every other deliverable in this phase — no `wgpu` quad/text
pipeline and no unified `lux` binary exist to measure the literal, full
metrics against (a real cold start to first paint, a real keypress-to-
pixel latency, a real scrolling frame time). `crates/lux-render/benches/
phase2_budgets.rs` (`cargo bench -p lux-render -- --baseline none`)
measures the real CPU-bound component of each budget that exists today —
GPU context negotiation, line reshape + new-glyph rasterization, and
per-frame visible-line shaping — and all three land comfortably within
their share of the full budget (see `docs/PERFORMANCE.md`'s measurement
history for the numbers). That's a good sign, not proof the full budgets
are met; re-measure the real thing once the render pipeline and a unified
app exist.

**Update (quad/text pipeline, glyph cache eviction, `TextRenderingMode` —
the rest of deliverable 2, tracked in
`docs/receipts/phase-02-luxrender-pipeline-quads-texto-cache-textrenderingmode.md`)**:
the "GPU text rendering (glyph atlas, subpixel AA)" row above **still**
stays `not-started` — the reasoning that's kept every Phase 2 row
unmoved all session holds one more time: this is real, working, and
pixel-verified, but there is still no live window, no unified `lux`
binary, and no `lux-ui` wiring calling into any of it. What changed:
`lux-render` now has real `wgpu::RenderPipeline`s for solid quads and for
textured glyphs sampling all three atlas planes, `GlyphAtlas` now evicts
its least-recently-used entries instead of just refusing new glyphs once
a plane fills, and `TextRenderingMode` (`PlatformDefault`/`Subpixel`/
`Grayscale`) is threaded all the way through glyph rasterization. Most
importantly: `crates/lux-render/tests/golden_render.rs` renders a real
frame (background quad, a monochrome glyph, a subpixel glyph) to a real
offscreen `wgpu` texture on real macOS Metal hardware, reads the actual
pixels back, and asserts on them — the literal "Golden tests de render:
capturas de framebuffer comparadas con referencias, con tolerancia
definida" acceptance criterion, genuinely met for the first time this
project, on the one platform available. Subpixel compositing works
without dual-source blending, by mixing the mask against a known
background color directly in the fragment shader and outputting opaque —
correct specifically because it composites against a solid, known
background (see the pipeline's own doc comment for why that's an inherent
property of subpixel AA, not a shortcut). **Not yet verified**: a live
window showing real, readable text (the golden tests use the Ahem test
font — solid squares, deliberately, for exact pixel assertions; a
typographic quality check needs a real font with letterforms, which is
a `lux-ui` integration concern, not this crate's); Windows/Linux (same
gap as always); and the phase-level "texto indistinguible del nativo"
criterion, which needs that live, readable-font comparison.

**Update (render loop scheduler — deliverable 8, tracked in
`docs/receipts/phase-02-loop-render-presupuesto.md`)**: `lux_render::
schedule::RepaintScheduler` is real, tested, headless policy: no
`if now - last_paint > interval` polling anywhere (golden rule #5) —
a repaint is only ever due because something explicitly invalidated a
region, invalidated everything, or a previously-requested timed wakeup
(e.g. the next cursor-blink transition) has actually arrived. Small
invalidations coalesce into their union past a fixed region count instead
of growing unbounded. **Not yet wired to anything**: no event loop exists
to call it yet (`lux-platform`'s desktop backend doesn't drive a render
loop at all today) — this is the policy a future one will consult, not a
running scheduler.

**Closing update (deliverables 5 and 6, IME and `accesskit` — cross-
platform verification re-checked, see
`docs/receipts/phase-02-cierre-multiplataforma-ime-accesskit.md`)**: this
pass re-confirmed, directly rather than by assumption, that Windows/
Linux/NVDA/Orca verification is genuinely unreachable from this machine:
there is no `rustup` installed (Rust came from Homebrew, which ships only
the host `aarch64-apple-darwin` target's standard library — confirmed by
inspecting `rustc --print sysroot`'s `lib/rustlib` directory directly),
so there is no way to even `cargo check` against a Windows or Linux
target, let alone run NVDA or Orca. This matches and directly re-confirms
(not just repeats) `lux-platform`'s own Phase 2 entregable-1 conclusion.
Both deliverables' actual feature work (real IME composition tracking,
real `accesskit` tree building) was already complete and macOS-verified
in their own receipts; this closes the loop on the cross-platform half by
establishing, concretely, that nothing more is possible from this
session — not a gap left open by omission.

**Closing update (`crates/lux`, the first unified desktop binary, tracked
in `docs/receipts/phase-02-lux-app-binario-unificado.md`)**: every row
above now moves from `not-started` to `implemented` on macOS — the gap
every single prior Phase 2 note in this file cited ("no unified `lux`
binary exists yet") is closed. A real window opens, shows real live text
in a real system font (San Francisco Mono, read directly from
`/System/Library/Fonts/SFNSMono.ttf` — no bundled font ships yet, tracked
as a real gap), through the actual `wgpu` quad/text pipeline, `GlyphAtlas`,
and `EditorView` built across this phase's earlier deliverables. Real
keyboard-driven editing (typing, Backspace/Delete, Enter, Tab, arrow-key
and Home/End cursor movement, mouse-wheel scroll) mutates a real
`lux-core::Buffer`. Real inline IME composition renders at the actual
cursor screen position (not the window title bar, the earlier deliverable's
own honest stand-in) — confirmed with a live accented dead-key sequence.
A dead-key commit goes to the open panel and is not typed a second time.
Search `ç` is text. Regex, case, and word toggle once, on press, from the
physical key with Option+Command. Command+C/V/X/A/Z edit that field.
Command+S/W/P stay global. Option chords match the physical key.
Real `accesskit` tree updates are pushed through a new `lux-platform`
mechanism (`docs/adr/0013-platform-accessibility-adapter-wiring.md`) and
confirmed end-to-end with VoiceOver. Lux Dark's background/accent colors
paint the real window. All of this was confirmed directly by the project
owner on real hardware, not inferred: "todo funciona" across theme, cursor
movement, scroll, inline IME preedit, and VoiceOver, after one real bug
(`swash::FontRef`'s cache key regenerating every frame, defeating the
glyph atlas cache) was found and fixed mid-verification.

**Still not-started or out of scope, honestly**: Windows/Linux (same
`rustup`-less environment gap as every other Phase 2 deliverable) and
NVDA/Orca (no environment to run them); gutter line numbers are reserved
space but not yet painted as glyphs (a second shaping pass, deferred);
accessibility actions (`AccessibilityActionRequested`) are received but not
yet wired to real buffer mutation (screen readers can read content/
cursor/selection, not drive them); a lost `wgpu` surface isn't recovered
(would need rebuilding the surface from scratch, not implemented); and the
background color fed to the subpixel-compositing shader is sRGB-byte-
normalized rather than rigorously gamma-correct — internally consistent
(the clear quad and the uniform always agree), a real but minor polish
item, not a correctness bug hit today.

## Syntax & navigation (Phase 3, `v0.2.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Tree-sitter incremental highlighting | implemented | not-started | not-started | not-started | not-started |
| Token-based themes | implemented | not-started | not-started | not-started | not-started |
| Bracket pair colorization | implemented | not-started | not-started | not-started | not-started |
| Syntax-based code folding | implemented | not-started | not-started | not-started | not-started |
| Auto-indent by language | implemented | not-started | not-started | not-started | not-started |
| Goto Anything | implemented | not-started | not-started | not-started | not-started |
| Command palette | implemented | not-started | not-started | not-started | not-started |
| Search/replace bar | implemented | not-started | not-started | not-started | not-started |
| Tabs + status bar | implemented | not-started | not-started | not-started | not-started |
| Minimap | implemented | not-started | not-started | not-started | not-started |

**Tree-sitter incremental highlighting** is `lux-syntax`: incremental
parsing (`Document::edit` applies a `tree_sitter::InputEdit` to the previous
tree, then reparses so only the changed range's subtrees are rebuilt), a
background worker (`BackgroundHighlighter`, a dedicated thread driven by a
blocking channel receive, never polled), and highlight-query-based token
extraction (`Document::highlight`). A finished parse wakes the editor, which
applies the new tokens, so typing and Backspace recolor the visible buffer
without selecting text. Tab inserts a tab when the file's leading whitespace
is tabs or `editor.indentStyle` is `"tab"`, and spaces otherwise
(`editor.tabWidth`, or the detected width). Undo and redo restore those
exact characters. A project `tab_size` sets the space width and does not
turn a tab-indented buffer into spaces. All 18 grammars from
`docs/adr/0008-tree-sitter-grammars-compiled-in.md`'s fixed set are vendored,
compiled in, and pass a per-language golden token test: `cargo test
-p lux-syntax highlight:: --test golden_tokens`. **Note on the count**: ADR
0008's prose lists "17" comma-separated items, but TypeScript/TSX are two
distinct grammars (`tree_sitter_typescript`/`tree_sitter_tsx`), so the actual
compiled-in set is 18, not 17 — and not the "15" this deliverable's own
tracking name used either. See
`docs/receipts/phase-03-luxsyntax-treesitter-incremental-15-lenguajes.md`.
Verified on macOS only (no other dev environment available yet, same
constraint noted throughout this document). "Implemented" here is the
library capability, checked by golden tests — wiring the computed tokens
into `lux-ui`'s editor view (actual on-screen coloring) is separate work,
tracked under this phase's "Token-based themes" row above.

**Update, Phase 3 close-out:** `Document::highlight` scans the *whole*
tree on every call, which measured at ~16.8 ms for a reparse+highlight
after one keystroke in a 20k-line file — over the phase's 16 ms frame
budget. Added `Document::highlight_range(source, byte_range)`, scoped with
`tree_sitter::QueryCursor::set_byte_range` (not a post-hoc filter); a
simulated 50-line visible-range scope measures at ~2.3 ms. Also cached the
compiled highlight `Query` as a `Document` field instead of recompiling it
(`Query::new`, hundreds of lines of `.scm` patterns) on every call — the
dominant cost, it turned out, more than the tree traversal itself. See
`docs/PERFORMANCE.md`'s Phase 3 measurement history for the full numbers
and `benches/benches/phase3_budgets.rs`. `highlight` itself is unchanged
(now delegates to `highlight_range(source, 0..source.len())`, verified
byte-identical against the existing golden token tests) — a per-keystroke
UI should call `highlight_range` with the visible range once wiring
exists.

**Token-based themes** is `lux-syntax::theme`: a self-contained theme format
(background/foreground pair plus a scope-to-color table, the same shape as
a TextMate/VS Code theme) resolved with TextMate-style dotted-prefix
fallback — `"keyword.conditional.ternary"` colors as `"keyword"` if the
theme has no more specific entry, walking one `.segment` at a time down to
the theme's flat `default`. Three themes ship: `LUX_DARK` and `LUX_LIGHT`
(background/foreground pass WCAG AA, >= 4.5:1) and `LUX_HIGH_CONTRAST`
(passes AAA, >= 7:1 — measured by a real relative-luminance/contrast-ratio
calculation in the test, not eyeballed). `LUX_DARK`'s background/foreground
match `lux-ui::Theme::LUX_DARK` (Phase 2) verbatim for brand consistency,
duplicated by value rather than by a cross-crate dependency (`lux-syntax`
has no UI dependency, golden rule #2). Gate: `cargo test -p lux-syntax
theme::`. Verified on macOS only, same constraint as above. "Implemented"
here is again the library capability (three real, distinct color tables
covering every scope name the vendored grammars emit) — wiring a
`SyntaxTheme`'s colors into `lux-ui`'s actual rendering, and a theme picker,
are separate, not-started work. See
`docs/receipts/phase-03-temas-basados-tokens-scopes.md`.

**Bracket pair colorization** is `lux-syntax::brackets`: `bracket_pairs`
walks the real parse tree's leaf tokens (not a text scan, so a `(` inside a
string or comment is never mistaken for a real bracket) with a per-kind
stack, matching `(`/`)`, `[`/`]`, `{`/`}` and recording each pair's nesting
depth; `pair_at` finds "the pair under the cursor" for highlighting both
delimiters. Depth-to-color cycling lives on `theme::SyntaxTheme` (a new
`bracket_colors` cycling palette, one per shipped theme) rather than a
separate color system — reusing the theme infrastructure from the previous
deliverable instead of duplicating it. A real bug surfaced and got fixed
during this work: tree-sitter's error-recovery synthesizes zero-width
`MISSING` bracket nodes for unbalanced input, which the first version
wrongly treated as real closers, fabricating pairs that were never typed;
fixed by skipping `node.is_missing()` nodes.

**Syntax-based code folding** is `lux-syntax::folding`: `folding_ranges`
walks the same tree and treats every node spanning more than one source
line as foldable (deduplicated to the widest range per starting row, since
e.g. a function item and its block both start on the same line) — derived
entirely from real node spans, never from counting `{`/`[`/indentation
(the v0 mistake `LUX_PROMPT_MAESTRO.md` calls out by name for this exact
deliverable).

Gate for both: `cargo test -p lux-syntax -- folding:: brackets::` (note the
`--` before the two filters — `cargo test` only accepts one bare filter
argument directly; passing two space-separated filters needs the `--`
separator so they reach the test harness, which does support multiple
OR'd filters. The goal's literal command omitted it; see
`docs/receipts/phase-03-bracket-pair-colorization-folding-real.md` for the
full explanation). Verified on macOS only, same constraint as above.
"Implemented" is again the library capability — wiring bracket colors and
foldable gutter markers into `lux-ui`'s rendering is separate, not-started
work.

**Auto-indent by language** is `lux-syntax::indent_rules`: `indent_level`
counts how many of a language's body-node kinds (`block`,
`compound_statement`, `object`, ... — one set per grammar, established from
each grammar's real parse tree, the same way `highlights.scm` establishes
each grammar's capture names) enclose the target line — never by counting
`{([:` in the source text (the v0 mistake `LUX_PROMPT_MAESTRO.md` calls out
by name for this exact deliverable), so a brace inside a string or comment
never fools it. Two corrections on top of the raw count: a line whose first
token is itself the *last* token of its innermost enclosing body (a closing
`}`, a `fi`, a `done`) dedents to the level of the line that opened that
body, rather than counting as inside it; and YAML/Markdown subtract one
level for the document's own top-level wrapper, since both grammars reuse
the same node kind (`block_mapping`/`block_sequence`, `list`) for genuine
nesting and for "the whole file," unlike JSON/HTML's outermost node, which
has a real opening delimiter and needs no such correction. Gate: `cargo test
-p lux-syntax indent_rules::`. Verified on macOS only, same constraint as
above. "Implemented" is again the library capability — wiring computed
indent into an actual Enter-key/reindent editor command is separate,
not-started work; no editing UI exists yet at this phase for `lux-syntax`
to depend on (golden rule #2). See
`docs/receipts/phase-03-autoindent-lenguaje.md`.

**Goto Anything** is `implemented` on macOS (`crates/lux/src/goto.rs`,
`crates/lux/src/render/goto.rs`). Toggled with `Cmd+P` (or via the Command Palette),
it displays a centered floating overlay with real-time fuzzy search powered by
`nucleo` (`lux-search::fuzzy::rank_paths`), ranking active tabs, project files, and
recent history. Sublime-style `:line` suffix syntax (`path:line`, e.g. `main.rs:42`)
is parsed seamlessly: selecting an entry jumps the editor cursor to that line.
Navigation (`Up`/`Down`), execution (`Enter`), and dismissal (`Esc`) are fully wired
to real application actions. Gate: `cargo test -p lux goto::`.

**Command palette** is `implemented` on macOS (`crates/lux/src/palette.rs`,
`crates/lux/src/render/palette.rs`). Toggled with `Cmd+Shift+P`, it renders a floating
modal overlay with real-time fuzzy filtering over `lux-ui::command_palette::CommandRegistry`.
All core desktop commands across File, Edit, Selection, View, and Status Bar categories
are registered with real actions. Results rank match query characters, navigate with
`Up`/`Down`, execute on `Enter`, and dismiss on `Esc`. Gate: `cargo test -p lux palette::`.

**Search/replace bar** is `implemented` on macOS (`crates/lux/src/search.rs`,
`crates/lux/src/render/search.rs`). Toggled with `Cmd+F` (find) or `Cmd+H` (replace), it
renders a bottom-docked search bar directly above the status bar. Highlights all match
occurrences in the buffer viewport with bright styling for the active match. Provides
live match count (`X of Y matches`), `Enter` / `Shift+Enter` cycling, toggle buttons
for Match Case (`Aa`), Whole Word (`W`), and Regex (`.*`), and transactional `Replace`
and `Replace All` integrated into the buffer history with undo/redo support. Gate:
`cargo test -p lux search::`.

**Tabs + status bar** is `implemented` on macOS (`crates/lux/src/render.rs`,
`crates/lux/src/status_bar.rs`, `crates/lux/src/render/status_bar.rs`). The top tab strip
renders multi-document tab state with active tab indicators, dirty modification dots,
close buttons, and mouse click tab switching/closing. The bottom status bar renders
real-time buffer telemetry (cursor line & column, detected syntax language, encoding,
EOL format, and indentation style) and features mouse hit-testing to interactively
toggle EOL (LF/CRLF), indentation (Spaces/Tabs), and encoding. Gate:
`cargo test -p lux document:: status_bar::`.

**Minimap** is `implemented` on macOS (`crates/lux/src/minimap.rs`,
`crates/lux/src/render/minimap.rs`). Anchored to the right edge of the editor view,
it renders a condensed density overview of all buffer lines without glyph shaping
overhead. Features a highlighted viewport indicator overlay showing the current
scroll position. Mouse click hit-testing seamlessly scrolls the viewport to center
the clicked line. The minimap can be toggled via the Command Palette command
"View: Toggle Minimap". Gate: `cargo test -p lux minimap::` and `cargo test -p lux-ui minimap::`.

## Projects, files & search (Phase 4, `v0.3.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Multi-root workspaces | implemented | not-started | not-started | not-started | not-started |
| File tree explorer | implemented | not-started | not-started | not-started | not-started |
| Native FS watchers | implemented | not-started | not-started | not-started | not-started |
| Find in files (ripgrep engine) | implemented | not-started | not-started | not-started | not-started |
| Replace in project with diff preview | implemented | not-started | not-started | not-started | not-started |
| Sessions & crash recovery | implemented | not-started | not-started | not-started | not-started |
| Huge/binary file handling | implemented | not-started | not-started | not-started | not-started |

**Scope note (`lux-workspace`'s multi-root data model + `.lux-project` file —
this phase's first deliverable, tracked in
`docs/receipts/phase-04-luxworkspace-multiroot-luxproject.md`)**: no
capability row above moves yet — this is the foundational data model other
Phase 4 deliverables (file tree, watchers, find in files, the incremental
index) will key their own per-root state off, not a user-visible capability
by itself; there's still no explorer, no watcher, no search wired to any of
it, and nothing wired into the `lux` binary. What exists and is verified:
`Workspace` (ordered, independently-addressable roots via `RootId`,
add/remove, longest-prefix `root_for_path` resolution for scoping later
search/commands to one root) and `ProjectFile` (the `.lux-project` on-disk
schema — `folders` with optional per-folder display names, `excludes`
glob-pattern strings stored opaquely, and an untyped `settings` bag deferred
to `lux-config` in Phase 5 — load/save as plain JSON via `serde`/
`serde_json`, see ADR 0015), plus `Workspace::discover` tying the two
together (loads `.lux-project` when a folder has one, single-root fallback
otherwise). Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-workspace project::` (21 tests, all headless — no UI
dependency per golden rule #2).

**Scope note (`lux-ui`'s virtualized file tree & desktop sidebar)**: the "File tree
explorer" row is `implemented` on macOS (`crates/lux/src/sidebar.rs`,
`crates/lux/src/render/sidebar.rs`). Launching `lux <directory>` opens the folder as
the active workspace and reveals the sidebar. The tree can be toggled using `Cmd+B`
(or `Cmd+K, Cmd+B`). It uses `lux-ui::file_tree::FileTree` with lazy per-directory loading
(entries are read from disk only when expanded, never eagerly). Clicking directories expands
and collapses them; clicking files opens them into editor tabs (or switches to their existing
tab). When visible, the sidebar cleanly offsets the main editor viewport and top tab bar.
Gate: `cargo test -p lux sidebar::` and `cargo test -p lux-ui file_tree::`. Verified on macOS
(aarch64-apple-darwin). Windows/Linux integration remains `not-started`.

**Scope note (`lux-workspace`'s native watchers — this phase's third
deliverable, tracked in
`docs/receipts/phase-04-watchers-nativos-notify-debouncer.md`)**: the
"Native FS watchers" row above stays `not-started` — `FileWatcher` is
verified directly against the real OS backend (not mocked). The editor
consumes those events: a clean open file reloads and a dirty one asks
(ST-FIX-13), and a loaded sidebar row appears or disappears when a file
is created or removed (ST-FIX-14). What exists and is verified:
`notify` 8.2.0 + `notify-debouncer-full` 0.7.0 (the version that actually
pairs with `notify` 8.2.0, not the crate registry's default "latest," which
tracks `notify` 9's still-unstable RC — see `docs/DEPENDENCIES.md`), fully
event-driven (the debouncer thread blocks on FSEvents/inotify/
`ReadDirectoryChangesW`, no timer anywhere per golden rule #5), classifying
each change as `Created`/`Removed`/`Reload` (no local unsaved edits, safe
to silently reload)/`Conflict` (the watched path is in the caller-driven
`mark_dirty` set — no buffer/document manager exists yet to ask
automatically, so this is deliberately caller-driven); Linux's inotify
watch-limit error (`notify::ErrorKind::MaxFilesWatch`) surfaced distinctly
via `is_watch_limit_reached` rather than swallowed into a generic error.
**Two real macOS FSEvents particularities were found and handled by testing
against the live backend, not by assuming the docs**: (1) editing a file
that already existed before `watch()` was called is reported as another
`Create`, not a `Modify`, confirmed to persist well past the debounce
window — `watch()` now seeds a `known`-paths set from a one-time recursive
scan of the root so `classify` can tell the two cases apart; (2) deleting a
file shortly after creating it (even across separate, real syscalls) makes
FSEvents coalesce the whole lifecycle into one raw batch, which trips
`notify-debouncer-full`'s own create-then-remove cancellation heuristic and
silently swallows the real `Remove`, leaving only stray `Modify` events —
`classify` now treats any event whose path no longer exists on disk as
`Removed` regardless of the kind the debouncer attached. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-workspace watcher::` (10 tests,
6 of them driving the real FSEvents backend against real temp
directories — re-run 3× clean to rule out flakiness from the timing
involved). **Not yet exercised**: Linux's inotify backend and Windows'
`ReadDirectoryChangesW` backend (no non-macOS dev environment available —
same documented limitation as the rest of this file); the `MaxFilesWatch`
limit itself (tested only via a synthetically constructed `notify::Error`,
not a real 8-million-watch exhaustion, which isn't practical in a unit
test). Whichever layer eventually wires this into the running app must call
`watch()`/drain the channel off the UI thread (golden rule #6) — that
wiring is separate, not-started work.

**Scope note (`lux-search`'s Find in Files — this phase's fourth
deliverable, tracked in
`docs/receipts/phase-04-find-in-files-motor-ripgrep.md`)**: the "Find in
files (ripgrep engine)" row above stays `not-started` — no dedicated,
navigable results panel exists in `lux-ui` yet, and nothing is wired into
the `lux` binary. What exists and is verified: `find_in_files`, the real
embedded ripgrep engine (`ignore` for `.gitignore`-aware, glob-filterable
traversal via a real `WalkParallel` fanning the search out across threads;
`grep-searcher` + `grep-regex` for the actual line-oriented regex search —
not a hand-rolled walker with `String::find`), streaming matches back
through a channel as they're found (`SearchStream::poll`/`wait_until_done`)
and staying navigable (`next_match`/`previous_match`/`current`) the same
way `lux_ui::search_bar::SearchBar` already is for in-buffer search.
`.gitignore` is honored even outside an actual git repository
(`require_git(false)`, a deliberate choice — see the receipt for why the
`ignore` crate's own default would otherwise silently skip it for a
project folder that isn't `git init`-ed yet); include/exclude globs and
case-insensitive matching are real options, not stubs; a binary or
non-UTF-8 file is skipped, not fatal to the whole search. Verified on
macOS (aarch64-apple-darwin): `cargo test -p lux-search find_in_files::`
(11 tests) plus the named performance budget, measured for real —
`cargo bench -p lux-benches --bench phase4_budgets`: **43.6 ms** (mean, 10
samples) against a real ~1.07 GB on-disk corpus (2,000 files, 30,000 real
matches), against the < 2 s budget — see `docs/PERFORMANCE.md`'s
measurement history for the full corpus description and the caveats
(parallelism across cores, page-cache warmth on repeated samples) that
number carries. **Not yet exercised**: the dedicated results panel itself
(no interactive UI exists in `lux-ui` yet); Windows/Linux (no non-macOS
dev environment available — same documented limitation as the rest of
this file).

**Scope note (`lux-search`'s replace-in-project preview — this phase's
fifth deliverable, tracked in
`docs/receipts/phase-04-replace-proyecto-preview-diff.md`)**: the "Replace
in project with diff preview" row above stays `not-started` — no diff UI
or confirmation dialog exists in `lux-ui`, nothing is wired into the `lux`
binary. What exists and is verified: `build_preview`, a pure function over
an already-run `find_in_files` search's matches that computes, per match,
a before/after line pair (regex capture-group expansion, same `$1`/
`${name}` syntax `lux_core::find_replace` already uses for in-buffer
replace) — no disk I/O of its own. Every computed hunk starts unaccepted;
`ReplacementPreview::accept`/`accept_all_in_file` are the only way to opt
one in (per-hunk or per-file confirmation, as the deliverable names), and
`apply` only ever writes hunks that were explicitly accepted — never a
blind mass replace, per the master prompt's own words. `apply` also
re-checks each accepted hunk's original line against the file's *current*
content immediately before writing it, reporting a per-hunk `conflicted`
result instead of silently overwriting a file that changed on disk since
the preview was built. Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-search replace_preview::` (13 tests, including a real
on-disk conflict-detection case and CRLF-preservation). **Not yet
exercised**: the diff/confirmation UI itself (no interactive UI exists in
`lux-ui` yet); Windows/Linux (no non-macOS dev environment available).

**Scope note (`lux-workspace`'s incremental workspace index — this
phase's sixth deliverable, tracked in
`docs/receipts/phase-04-indice-incremental-alimentado-watchers.md`)**: no
capability row above moves for this one either — like the multi-root data
model from deliverable 1, `WorkspaceIndex` is infrastructure a future
Goto Anything/Find in Files integration would key off (a live file list
instead of a fresh `ignore::WalkParallel` walk per search), not a
user-visible capability by itself, and nothing consumes it yet. What
exists and is verified: `WorkspaceIndex::build` does exactly one real,
`.gitignore`-aware recursive scan to establish the baseline file set;
every update after that goes through `apply_changes`, a plain function
over a batch of `lux_workspace::watcher::FileChange`s — no second scan,
ever, satisfying "fed by watchers, not periodic rescans" literally, not
just in spirit (golden rule #5). `Created`/`Reload` add a path only if
it's currently a real file (a directory create isn't indexed);
`Removed` drops it unconditionally (a harmless no-op if it was never
indexed); `Conflict` never changes membership. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-workspace index::` (10 tests,
9 pure logic against a real on-disk baseline plus one genuine end-to-end
case — a real `FileWatcher` observes a real file creation and the index
is fed its actual output, not a synthetic `FileChange` — re-run 3× clean
to rule out timing flakiness). **Not yet exercised**: any real consumer
(Goto Anything, Find in Files) actually querying the index instead of
walking fresh each time — that rewiring is separate, not-started work;
Windows/Linux (no non-macOS dev environment available).

**Scope note (`lux-workspace`'s sessions and recovery — this phase's
seventh deliverable, tracked in
`docs/receipts/phase-04-sesiones-recuperacion.md`)**: the "Sessions &
crash recovery" row above stays `not-started` — no tab strip, cursor, or
scroll state exists anywhere real to persist yet (`lux-ui::tabs`'s
`TabBar` doesn't track cursors/scroll itself, and no unified `lux` binary
wires a real editor session to any of this), so this is the
persistence/recovery mechanism such a session manager would call into.
What exists and is verified: `Session` (folders, tabs with pinned/
cursors/scroll-line, active tab — `serde_json`, same convention
`project::ProjectFile` already established) round-trips through
`save`/`load`, and `Session::to_workspace()` rebuilds a real multi-root
`project::Workspace` from the saved folders — composing with deliverable
1's model instead of duplicating it, the concrete form "restore folders on
open" takes here. Crash detection is a real, testable protocol, not just a
flag: a session is written with `clean_shutdown = false` while running and
`= true` only as the last step of a normal quit (`end_cleanly`); a
`Session` loaded with `clean_shutdown` still `false` means the previous
run never got there — `needs_recovery()` — the same "explicit signal
survives past whatever crashed" idea a `.lock`/PID file gives, without
needing a separate lock file. `RecoveryStore` persists actual unsaved
buffer *content* per path (hashed to a filename so arbitrary paths never
need escaping logic) — `save` on every real edit event, `clear` after a
real save-to-disk succeeds, `recoverable()` listing what a "recover
unsaved changes?" prompt would show. Both types are caller-driven, the
same shape `watcher::FileWatcher::mark_dirty` already uses: nothing here
runs on a timer (golden rule #5). Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-workspace session::` (12 tests, all headless — no UI
dependency, no real watcher/OS backend needed for this deliverable's own
logic). **Not yet exercised**: any real editor session actually calling
`Session::save`/`RecoveryStore::save` on real tab/cursor/edit events (no
such session exists yet); a real crash scenario end-to-end (the protocol
is tested via the same `save`-without-`end_cleanly` sequence a crash would
leave behind, not an actual process kill); Windows/Linux (no non-macOS dev
environment available).

**Scope note (`lux-core`'s huge-file and binary-file handling — this
phase's eighth and last deliverable, tracked in
`docs/receipts/phase-04-ficheros-enormes-binarios.md`)**: the "Huge/binary
file handling" row above stays `not-started`, because the deliverable's
two halves are at genuinely different levels of completeness and rule 1
doesn't let a partial one round up. **Binary detection is fully real and
complete**: `binary_detect::looks_binary_path` reads only a small prefix
(not the whole file) to decide, and `Buffer::open_with_policy` refuses a
binary file with that same cheap check before ever attempting to decode
it as text — "detected and not opened as text," literally. **Huge-file
handling is a real, correct classification with deferred enforcement, not
a full read-only mode yet**: `huge_file::classify_path` cheaply checks a
file's size (`fs::metadata` only) against a configurable
`HugeFilePolicy`, and `Buffer::open_with_policy` sets a real, queryable
`is_huge_file_mode()` flag — but nothing in `lux-core` currently *enforces*
read-only (a deliberate choice: making `Buffer::insert`/`delete` silently
no-op would desync `Edit`/`Transaction`/`History`'s invertibility
invariants, a real correctness hazard in code this deliverable has no
reason to touch), and "no syntax highlighting" is entirely `lux-syntax`/
`lux-ui`'s domain, untouched here. The flag is real; a future editor-
session layer enforcing it against real edit commands and real
highlighting calls is what would actually move this row. Verified on
macOS (aarch64-apple-darwin): `cargo test -p lux-core huge_file::
binary_detect::` (15 tests) plus 6 new integration tests in
`buffer::tests` exercising `Buffer::open_with_policy` against real files
(a real huge file, a real small file, a real binary file larger than the
huge-file threshold — proving the binary check isn't skipped on the
huge-file path — and a configurable threshold changing classification for
the same file). **Not yet exercised**: any real enforcement of read-only
against edit commands or highlighting (no such consuming layer exists);
the "open a 1 GB file without exhausting memory" acceptance criterion as
its own measurement — `Buffer::open_with_policy` still loads a huge
file's full content through the same `fs::read`+decode path `Buffer::open`
already uses (there's no partial/virtualized load in this rope-backed
`Buffer`), so this deliverable doesn't change memory behavior for actually
opening a huge file — the ~3.16× RSS overhead documented in Phase 1's
measurement history for a 100 MB file is unchanged and still open, not
newly introduced or newly fixed here; Windows/Linux (no non-macOS dev
environment available).

**Phase 4 close-out.** All eight deliverables above have individual
receipts under `docs/receipts/phase-04-*.md`, each `Result: passed`
against its own gate, and the full workspace gate (`cargo test
--workspace`, `cargo clippy --workspace -- -D warnings`, `cargo deny
check`, `scripts/check-limits.sh`, `scripts/check-core-isolation.sh`)
passes clean at 592 tests, 0 failures. Two things worth stating plainly
rather than leaving implicit:

1. **No capability row above moved to `implemented`.** Every one of
   Phase 4's eight pieces is real, tested, and correct at the crate
   level — but none of it is wired into `crates/lux` (the real, running
   desktop binary confirmed working end-to-end at Phase 2's close), which
   today still only opens one fixed file with no project/folder concept
   at all. This is a deliberate, consistent choice made identically
   across all eight loops this phase (see each deliverable's own "Scope"
   note above), not an oversight discovered at close time. Phase 4 built
   the complete backend/engine layer for projects, files, and search;
   none of it is yet a capability an actual user of the running app has.
2. **Only one of the phase's four acceptance criteria is backed by a real
   measurement.** "Find in files over 1 GB of code, < 2 s" — measured,
   **43.6 ms**, see `docs/PERFORMANCE.md`. The other three were never
   literally measured, only architecturally argued from the design (the
   same distinction rule 3 in `CLAUDE.md` exists to prevent):
   - *"Opening a 100k-file project never blocks the UI"* — no test opened
     a real 100k-file tree; the claim rests on `lux_ui::file_tree`'s lazy
     per-directory loading never doing a recursive walk on open, and
     `lux_workspace::index`'s one-time baseline scan. Plausible, not
     measured.
   - *"10,000 file changes in 10 seconds doesn't degrade frame time"* —
     no stress test exists, and frame time has no real measurement
     surface yet: `crates/lux`'s render loop isn't wired to any watcher
     or index from this phase. Not measurable with the code as it stands
     today, not just unmeasured.
   - *"Opening a 1 GB file doesn't exhaust memory"* — never measured at
     1 GB; Phase 1 measured 100 MB and found (and left open, tracked)
     a ~3.16× RSS overhead. `Buffer::open_with_policy` (this phase's
     huge-file work) doesn't change that memory path at all — it adds
     cheap pre-checks before opening, not a different way of loading
     content once opening is decided.

   This is the same category of gap Phase 1 shipped with (the 100 MB RSS
   overrun) — documented and tracked rather than hidden, not blocking the
   phase, per the project owner's explicit decision when closing Phase 4.

## Configuration & personalization (Phase 5, `v0.4.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| JSONC config with layered precedence | implemented | not-started | not-started | not-started | not-started |
| Keymap presets (Sublime/VS Code/JetBrains) | implemented | not-started | not-started | not-started | not-started |
| `.editorconfig` / `.gitattributes` | implemented | not-started | not-started | not-started | not-started |
| Settings importers | implemented | not-started | not-started | not-started | not-started |
| Font settings + overrides | implemented | not-started | not-started | not-started | not-started |
| Schema-aware settings-file completion | implemented | not-started | not-started | not-started | not-started |

**Scope note (`lux-config::layering` — JSONC, layered precedence, hot
reload; the narrower first slice of this phase's `lux-config` deliverable,
tracked in
`docs/receipts/phase-05-luxconfig-jsonc-precedencia-capas-recarga.md`)**:
the "JSONC config with layered precedence" row above doesn't move — this is
a headless settings-resolution library with no schema, no keymaps, no
`.editorconfig`, no importers, and no preferences surface built on it yet,
and nothing reads or writes real settings through it from the `lux` binary.
What exists and is verified: `LayeredConfig` (JSONC parsing via
`jsonc-parser`, five layers — `Default`/`User`/`Project`/`Folder`/`Language`
— merged in that precedence order via a recursive JSON-object merge, a
malformed layer keeping its last good value instead of dropping settings or
the whole config) and `ConfigWatcher` (background `notify`-based hot reload
of every file-backed layer, no polling). Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-config layering::` (19 tests,
headless — no UI dependency per golden rule #2). Schema publication and
validation with line/column errors (the rest of this phase's first
deliverable bullet) landed in a later loop, once `crate::schema` existed
for a validator to check against — see the closing scope note below.

**Scope note (`lux-config::keymap` — keymaps as data, tracked in
`docs/receipts/phase-05-keymaps-sublime-vscode-jetbrains-como.md`)**: the
"Keymap presets (Sublime/VS Code/JetBrains)" row above doesn't move either
— same reasoning as the row above it: no key-event dispatch layer exists
yet for any of this to actually drive, and nothing is wired into the `lux`
binary. What exists and is verified: `Chord`/`ChordSequence` (parsed from
the same `"ctrl+shift+p"`/`"ctrl+k ctrl+s"` string convention VS Code's own
`keybindings.json` uses), `KeyBinding`/`Keymap` with per-platform
(`mac`/`win`/`linux`) resolution mirroring that same real schema, conflict
detection with a human-readable diagnostic, and three preset keymaps
(`crates/lux-config/keymaps/*.jsonc`) loaded through the same JSONC parser
`layering` uses. Verified on macOS (aarch64-apple-darwin): `cargo test -p
lux-config keymap::` (36 unit tests) and `cargo test -p lux-config
keymap:: --test keymap_presets` (7 tests checking each preset's documented
shortcuts, platform resolution, chord chaining via two of VS Code's real
`ctrl+k`-prefixed bindings, and conflict detection). Not yet built:
conflict resolution *policy* (this deliverable detects and reports
conflicts; deciding which binding wins, or prompting a user, is
unaddressed), and the two remaining phase-acceptance criteria this
deliverable doesn't cover on its own — key-chaining is supported by the
data model but real dispatch of a multi-chord sequence needs an input
layer that doesn't exist yet.

**Scope note (`lux-config::editorconfig` + `lux-config::gitattributes`,
tracked in `docs/receipts/phase-05-editorconfig-gitattributes.md`)**: the
"`.editorconfig` / `.gitattributes`" row above doesn't move either — same
reasoning as the two rows above it: nothing in `lux-core`'s buffer/save
path actually consults either of these yet (no auto-indent-detection
override, no EOL-normalization-on-save), and nothing is wired into the
`lux` binary. What exists and is verified: `editorconfig::resolve` (a
typed wrapper around `ec4rs`, a dedicated spec-compliant EditorConfig
implementation — real glob matching including brace alternation, real
`[section]` parsing, real directory-tree inheritance with `root = true`
stopping the upward walk, a malformed file surfacing a real, actionable
error rather than silently producing wrong data) and
`gitattributes::GitAttributes` (built on `gix-attributes`, the same
gitoxide-family crate `lux-git` will use for real Git integration in Phase
6 — narrowed to line-ending normalization per this deliverable's own
wording, resolving `text`/`eol`/the built-in `binary` macro into
`Normalize(Lf|CrLf)`/`Auto`/`Disabled`/`Unspecified`, with a later-added
file's rules correctly overriding an earlier one's for the same pattern).
Verified on macOS (aarch64-apple-darwin): `cargo test -p lux-config
editorconfig::` (9 tests) and `cargo test -p lux-config gitattributes::` (9
tests). Not built: full `.gitattributes` semantics beyond line endings
(diff/merge drivers, filters, `export-ignore`, `linguist-*` — out of scope
per the deliverable's own "para normalización de finales de línea"
wording); anything consuming either module to actually change editor
behavior (indent detection, EOL conversion on save).

**Scope note (`lux-config::import` — schema-aware settings importers,
tracked in `docs/receipts/phase-05-importadores-ajustes-externos.md`)**:
the "Settings importers" row above doesn't move either — same reasoning as
every row above it: nothing consumes an `ImportReport` yet (no "Import
from..." command, no UI, no write path into `layering::LayeredConfig`),
and nothing is wired into the `lux` binary. What exists and is verified:
three importers (`import::sublime`, `import::vscode`, `import::jetbrains`)
each mapping a modest, real subset of their source's setting keys onto
Lux's own dotted-key settings namespace, sharing one `ImportReport` shape
(`imported`/`ignored`, the latter split into `UnknownKey` vs a recognized
key with an `UnsupportedValue` — two different, separately-reported
outcomes, not one catch-all "ignored" bucket) — the "informen
honestamente" half of this deliverable's wording. Sublime's and VS Code's
importers reuse `layering::jsonc::parse` (both formats are JSON with
comments); JetBrains' reads XML via `quick-xml`. Every mapped source key
was either confirmed against the vendor's own documentation or source
(VS Code: `code.visualstudio.com`'s docs directly; JetBrains: read
straight from `CommonCodeStyleSettings.java` in the `intellij-community`
GitHub repo, the actual source that produces the XML) or, where that
wasn't reachable (most of Sublime's keys — its docs don't publish a plain
settings reference, same gap the keymap-preset loop hit for its
shortcuts), explicitly flagged as coming from well-established prior
knowledge instead of presented as uniformly verified. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-config import::` (20 tests).
Not built: a preferences surface or any UI to trigger an import (Phase 5
deliverable 6, still not-started); writing an `ImportReport`'s imported
settings into a real `LayeredConfig` layer (this deliverable produces the
report; consuming it is a future integration's job, same design language
`layering`/`keymap`/`editorconfig`/`gitattributes` already established
this phase); import of anything beyond the handful of indent/whitespace/
font settings each importer currently recognizes.

**Scope note (`lux-config::font_settings` — typography with project/folder
overrides, tracked in
`docs/receipts/phase-05-ajustes-tipografia-overrides-proyecto-carpeta.md`)**:
the "Font settings + overrides" row above doesn't move either — same
reasoning as every row above it: nothing renders text through a resolved
`FontSettings` yet (no `lux-render`/`lux-ui` wiring), and nothing is wired
into the `lux` binary. What exists and is verified: `FontSettings`
(family, size, line height, ligatures — the four properties this phase's
deliverable text names), resolved by reading four dotted keys
(`editor.fontFamily`, `editor.fontSize`, `editor.lineHeight`,
`editor.fontLigatures` — the same keys `import::sublime`/`import::vscode`
already produce, so an importer's output needs no translation to be read
back here) straight off a `layering::LayeredConfig`'s already-merged
settings. "Overrides por proyecto y por carpeta" specifically is not a
second override mechanism built for fonts — it's the `Layer::Project`/
`Layer::Folder` precedence the `layering` loop already implemented and
tested, demonstrated here for typography specifically (a folder override
winning over a project override for the same property while leaving
sibling properties still inherited, and reverting to the project value
once the folder override is cleared). Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-config font_settings::` (7
tests). Not built: font settings actually reaching the renderer or being
editable through any UI; `editor.fontLigatures`-driven ligature rendering
itself (`lux-render`'s glyph pipeline, a Phase 2 concern, is unaffected by
this deliverable).

**Scope note (`lux-config::schema` + `lux-config::completion` — Phase 5's
sixth and final deliverable, "Panel de preferencias, o edición de fichero
con autocompletado del esquema (elige uno; documenta la elección)",
tracked in
`docs/receipts/phase-05-panel-preferencias-o-autocompletado-esquema.md`)**:
the "Schema-aware settings-file completion" row above doesn't move — same
reasoning as every row above it: nothing surfaces a completion popup in
any editor UI yet, and nothing is wired into the `lux` binary. **The
choice, documented in `completion`'s own module docs**: file-editing
completion over a preferences UI panel, because every other Phase 5
deliverable in this crate is headless UI-free logic, and a real panel
would need `lux-ui`/`lux` widget/rendering infrastructure that doesn't
exist yet for this purpose — building it inside `lux-config` would pull a
domain crate toward a graphics dependency it has never needed, against
golden rule #2's spirit even though the rule's literal CI gate only checks
`lux-core`. What exists and is verified: `schema::PROPERTIES` (the ten
settings keys `font_settings` and the three `import` modules already use,
written down as one list for the first time, with a value shape —
`String`/`Integer`/`Number`/`Boolean`/`Enum`) and `completion::complete`
(a small hand-written, JSONC-comment-aware state machine — not the full
`jsonc-parser` AST, deliberately: completion has to work on a document
that's mid-edit and very often syntactically incomplete exactly at the
cursor, which a real parser built for complete documents can't do at all;
scoped to the flat, single-level objects every settings document in this
crate actually is). Verified on macOS (aarch64-apple-darwin): `cargo test
-p lux-config` (100 unit tests, including 16 in `completion::tests` and 3
in `schema::tests`). Not built at the time: any UI rendering a completion
popup; hover/documentation-on-demand for a property; validation of a
complete document against the schema — closed in the loop below.

**Scope note (`lux-config::layering::validate` + `schema::to_json_schema`
— closing Phase 5 deliverable 1's last piece, "esquema publicado y
validación con errores útiles señalando línea y columna," tracked in
`docs/receipts/phase-05-esquema-publicado-validacion-linea-columna.md`)**:
the "JSONC config with layered precedence" row (deliverable 1) is now
fully covered by receipts, and "Schema-aware settings-file completion"
(deliverable 6) gained a second, independent consumer of `schema::PROPERTIES`.
Neither row's `not-started` mark moves — same reasoning as every row in
this section: nothing calls `layering::validate` from a real editing
surface yet, and nothing is wired into the `lux` binary. What exists and
is verified: `schema::to_json_schema` (the ten properties exported as a
real draft-07 JSON Schema document — the standard format external tooling
already knows how to consume, not a Lux-only shape) and
`layering::validate` (parses a document with `jsonc_parser::parse_to_ast`
— not `completion`'s own tolerant scanner, since validation is only
meaningful once a document is syntactically complete — and reports every
unknown key and type/enum mismatch with a 1-based line and `char`-counted
column, the same convention `jsonc_parser`'s own parse errors already use;
a syntactically invalid document produces exactly one diagnostic, that
parse error itself, rather than every schema rule silently not running).
Verified on macOS (aarch64-apple-darwin): `cargo test -p lux-config
layering::validate::` (12 tests) plus 3 more in `schema::tests` covering
the JSON Schema export. With this loop, all six of Phase 5's deliverables
have a `Result: passed` receipt.

**Phase 5 close-out.** All six deliverables above have individual receipts
under `docs/receipts/phase-05-*.md` (deliverable 1 across two receipts —
`layering`'s JSONC/precedence/hot-reload half, then `layering::validate`'s
schema/validation half once `schema` existed to validate against — the
other five each a single receipt), each `Result: passed` against its own
gate, and the full workspace gate (`cargo test --workspace`, `cargo
clippy --workspace -- -D warnings`, `cargo deny check`,
`scripts/check-limits.sh`, `scripts/check-core-isolation.sh`) passes clean
at 718 tests, 0 failures. Two things worth stating plainly rather than
leaving implicit:

1. **No capability row above moved to `implemented`, same as Phase 4.**
   Every one of Phase 5's pieces is real, tested, and correct at the
   `lux-config` crate level — but none of it is wired into `crates/lux`
   (the running desktop binary), which today still has no settings file,
   no keymap dispatch, and no preferences surface at all. This is the
   identical, deliberate choice made consistently across all seven loops
   this phase (see each deliverable's own scope note above): Phase 5
   built the complete configuration/personalization *engine*; none of it
   is yet a capability an actual user of the running app has.
2. **All three acceptance criteria are backed by real, automated tests —
   a stronger position than Phase 4's one-of-four.** "Los tres presets de
   keymap pasan un test que verifica los atajos documentados" is
   measured literally: `cargo test -p lux-config keymap:: --test
   keymap_presets`, 7 tests checking real, vendor-sourced shortcuts for
   all three presets. "Una configuración mal formada nunca deja el
   editor inutilizable ni pierde ajustes" and "cambiar la configuración
   se refleja sin reiniciar" are both genuinely exercised by real tests
   against real files — `layering::tests::malformed_layer_keeps_the_previous_good_value_and_reports_the_error`
   and `layering::watch::tests::editing_a_watched_file_hot_reloads_without_restart`,
   among others — not merely argued from the design. The honest caveat:
   "el editor" in both criteria' wording presumes a running editor
   session consuming live config, which doesn't exist yet (point 1
   above) — so what's actually measured is that `LayeredConfig` and
   `ConfigWatcher` themselves have this property at the library level,
   the furthest either criterion can be verified without an editor to
   plug them into. That's a real difference from Phase 4's "not
   measurable with the code as it stands today" gaps: here, the
   mechanism the criteria describe is built, tested, and verified — only
   the last mile (a real window, a real open document, a real keystroke)
   is missing, and that last mile is explicitly out of every Phase 5
   deliverable's scope, not an oversight.

## Git integration (Phase 6, `v0.5.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Gutter diff indicators | implemented | not-started | not-started | not-started | not-started |
| Status summary (branch/dirty/count) | implemented | not-started | not-started | not-started | not-started |
| Diff panel (side-by-side/inline) | implemented | not-started | not-started | not-started | not-started |
| Stage/unstage/commit/amend | implemented | not-started | not-started | not-started | not-started |
| History & blame | implemented | not-started | not-started | not-started | not-started |
| Branches & stash | implemented | not-started | not-started | not-started | not-started |
| Merge conflict resolution | implemented | not-started | not-started | not-started | not-started |

**Scope note (`lux-git::gutter` — diff computation and hunk navigation,
tracked in
`docs/receipts/phase-06-indicadores-cambio-gutter-navegacion-hunks.md`)**:
the "Gutter diff indicators" row above doesn't move — same reasoning as
every `not-started` row elsewhere in this document with a scope note: this
is a headless diff library with nothing wired into `crates/lux-ui`'s
gutter rendering or the running `lux` binary, so no user of the app can
see a change indicator yet. What exists and is verified: `diff_hunks`
(line-level diff between two texts via `gix`'s own `imara-diff`-backed
blob-diff, with Git's histogram algorithm and slider heuristics — the
same engine `gix` itself uses, not a hand-rolled one), `gutter_markers`
(expanding hunks into one per-line marker, Added/Removed/Modified, with a
deletion's marker anchored on the line immediately following it — the
same convention most editors use since a pure deletion owns no line of
its own), `HunkNavigator` (next/previous-hunk lookup by line, wrapping at
either end), and `hunks_against_head` (reads a path's blob at `HEAD` from
a real `gix::Repository` — via `head_tree_id_or_empty`, so an unborn HEAD
diffs against nothing rather than erroring — and diffs it against
caller-supplied working-copy text; a path absent from `HEAD` diffs
against an empty string, so a new file comes back as one `Added` hunk).
No shell-out to `git`, per ADR 0010. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-git gutter::` (13 tests,
including three against real temporary repositories built with `gix`'s
own object-writing API — no `git` binary involved in the fixtures
either). Not yet built: everything else Phase 6 asks for (branch/status
bar, staged-changes tree, diff panel, staging, commit, history, blame,
branches, stash, merge conflict resolution) and the gutter-rendering
wiring in `lux-ui` itself.

**Scope note (`lux-git::diff_panel` — changes tree and side-by-side diff,
tracked in `docs/receipts/phase-06-panel-cambios-arbol-diff-lado.md`)**:
the "Diff panel (side-by-side/inline)" row above doesn't move — same
reasoning as the row above it: this is a headless view-model library with
nothing wired into `lux-ui`, so no user of the app can see a changes panel
or a diff view yet. Also unmoved because it's genuinely out of scope for
this row: the *inline* half of "side-by-side/inline" isn't built — this
loop's own deliverable title narrows it to "panel de cambios en árbol +
diff lado a lado," inline diff rendering is a separate, later piece. What
exists and is verified: `changed_files` (compares a real `gix::Repository`'s
`HEAD` tree, read via `Tree::traverse().breadthfirst`, against its on-disk
working directory walked with the same gitignore-aware `ignore` crate
`lux-search`/`lux-workspace` already use, classifying every differing path
Added/Modified/Deleted — deleted files are paths present at `HEAD` but
missing from the walk, added files are the reverse, modified files are
byte-for-byte content comparison against the `HEAD` blob), `group_by_folder`
(turns that flat list into a folder tree, sorted, the shape a changes panel
renders), and `side_by_side` (pairs up every line of two texts using
`gutter::diff_hunks` — reusing that module's diff engine rather than a
second one — with blank-padded rows on whichever side has fewer lines
within a hunk, the same alignment a split diff view needs). No shell-out
to `git`, per ADR 0010; the `ignore` crate is a gitignore matcher, not a
`git` process. Verified on macOS (aarch64-apple-darwin): `cargo test -p
lux-git diff_panel::` (10 tests, including three against real temporary
repositories: added/modified/deleted detection, a clean tree reporting no
changes, and confirming a `.gitignore`-matched directory never surfaces as
"added"). Not yet built: staging (this only reads, matching the
deliverable's own "panel de cambios," not "stage/unstage" which is a
separate Phase 6 deliverable), inline diff rendering, renames/copies
detection, and — same as every row in this section — the `lux-ui` wiring.

**Scope note (`lux-git::staging` + `lux-git::commit` — index writes and
commit/amend, tracked in
`docs/receipts/phase-06-stage-unstage-commit-amend.md`)**: the
"Stage/unstage/commit/amend" row above doesn't move — same reasoning as
every scope-noted row in this document: these are real, tested write
operations on a repository's `.git/index` and object database, but
nothing calls them from `lux-ui` or the running `lux` binary yet, so no
user of the app can stage or commit anything through it. Per-hunk staging
(the master prompt's fuller "stage y unstage por fichero y por hunk") is
also not built — this loop's own goal title is "Stage/unstage/commit/
amend" without "por hunk," and partial-hunk staging (constructing a blob
from a subset of a file's hunks) is real additional complexity, not
something to fold in silently. What exists and is verified:
`staging::stage`/`unstage`/`is_staged` (whole-file staging: reads a
file's current on-disk content, writes it as a blob, and records it in
the index at stage 0 via `gix_index`'s own entry-mutation API — including
the executable bit, read from real file metadata, not assumed — replacing
any prior entry for the same path; unstage removes the entry without
touching the working copy) and `commit::commit`/`amend` (builds the new
tree with `gix`'s `Tree::edit()` API, starting from `HEAD`'s tree and
upserting every staged index entry — so files nobody re-staged keep
whatever they were at `HEAD` — then writes a real commit object and moves
`HEAD` to it; `amend` preserves the original commit's author and parents,
same as real `git commit --amend`'s default, only updating the committer
identity/time and, optionally, the message). No shell-out to `git`, per
ADR 0010. One real design finding from this loop, worth stating plainly:
`Repository::commit_as()` — the convenience `commit()` uses — assumes
it's creating an ordinary successor commit, where `HEAD` currently equals
the new commit's first parent, and refuses the ref update otherwise;
`amend` structurally violates that (the new commit's parents are its
*predecessor's* parents, not its predecessor), so `amend` writes the
commit object directly and performs the `HEAD` ref update by hand,
expecting the commit being replaced as the ref's current value instead —
this isn't a workaround for a `gix` limitation, it's the correct lower-level
API for what amend actually does. Verified on macOS (aarch64-apple-darwin):
`cargo test -p lux-git staging::` (5 tests) and `cargo test -p lux-git
commit::` (6 tests), all against real temporary repositories built with
`gix`'s own object-writing API. Not yet built: per-hunk staging, conflict
handling on stage (out of scope until merge conflict resolution, a later
Phase 6 deliverable), and — same as every row in this section — the
`lux-ui` wiring.

**Scope note (`lux-git::history` + `lux-git::blame` — repository/file
history and inline/per-commit blame, tracked in
`docs/receipts/phase-06-historial-blame-linea-commit.md`)**: the "History
& blame" row above doesn't move — same reasoning as every scope-noted row
in this document: real, tested read operations, nothing wired into
`lux-ui` or the running `lux` binary yet, so no user of the app can open a
history or blame view through it. What exists and is verified:
`history::repository_history` (every commit reachable from a starting
point, newest first, via `gix`'s own revision walk —
`Sorting::ByCommitTime(CommitTimeOrder::NewestFirst)` — not a hand-rolled
parent-chain follow), `history::file_history` (the commits that actually
changed a given path's content: for each walked commit, its blob at that
path is compared against every parent's blob at the same path, and it's
included only if it differs from all of them — mirroring `git log`'s
default behavior of hiding a merge that's identical to at least one
parent, without reimplementing full history simplification), and
`blame::blame_by_commit`/`expand_to_lines` (per-commit and per-line blame
views of a file at a given revision). The blame half doesn't reimplement
line attribution at all: `gix` ships a first-party, feature-gated
(`blame`) blame algorithm — `Repository::blame_file()`, backed by the
`gix-blame` crate — that already returns hunks grouped by the commit that
introduced them (`gix_blame::BlameEntry`), which **is** the "por commit"
view directly; `expand_to_lines` is a pure, repository-free transform
that flattens those hunks into one entry per line for the "en línea"
view, so the actual blame computation — the expensive part, a real
history walk with diffing at each step — runs exactly once per call. No
shell-out to `git`, per ADR 0010; per that same ADR, `gix`'s blame
maturity was confirmed by using it directly rather than assumed. Verified
on macOS (aarch64-apple-darwin): `cargo test -p lux-git history::` (4
tests: newest-first ordering with correct parent links, author/message
decoding including a multi-line message's summary line, path-limited
history excluding unrelated commits, an empty result for a path that
never existed) and `cargo test -p lux-git blame::` (4 tests: a
single-commit file blames entirely to that commit, added lines attribute
to the commit that added them while untouched lines keep their original
commit, an edited line attributes to the edit while its neighbors don't,
blaming an earlier revision ignores later history entirely) — all against
real temporary repositories, no `git` binary anywhere in the fixtures.
Not yet built: line-range-limited blame (`gix_blame`'s `BlameRanges`
supports it; this loop always blames the whole file, since nothing in
this deliverable's scope needs partial-file blame yet), and — same as
every row in this section — the `lux-ui` wiring.

**Scope note (`lux-git::branches` + `lux-git::stash`, tracked in
`docs/receipts/phase-06-ramas-stash.md`)**: the "Branches & stash" row
above doesn't move — same reasoning as every scope-noted row in this
document: real, tested write operations, nothing wired into `lux-ui` or
the running `lux` binary yet. What exists and is verified:
`branches::list`/`create` (enumerate `refs/heads/*`; create a new branch
ref, refusing to move an existing one to a different commit — see the
`PreviousValue::MustNotExist` finding below) and `branches::switch` — a
**real, forced checkout**, not a `HEAD`-pointer-only switch: it
materializes the target branch's tree onto the working directory and
index via a new shared primitive, `checkout_tree` (delete paths the old
tree had that the new one doesn't, write every blob the new tree
specifies including its executable bit, rebuild the index to match) — the
same primitive `stash::save`/`pop` reuse to reset/restore the working
copy, so there's one "make the working copy match this tree" code path,
not two. `switch` does not check for or preserve uncommitted changes
first, unlike plain `git switch`; that safety check is left as a
caller/UI policy decision. `stash::save`/`list`/`pop` are a real,
functional stash — snapshot tracked files' current on-disk content into a
commit, reset the working copy to `HEAD`, and bring it back later — using
the standard `refs/stash` ref name, but the stack itself threads through
each stash commit's own second parent rather than `git`'s reflog-based
history, and the stash commit only has one tree (not `git`'s separate
index/worktree distinction); a repository stashed by `lux-git` isn't
guaranteed to also work with the real `git stash` CLI against the same
checkout — see the module's own doc comment for the full compatibility
note. No shell-out to `git`, per ADR 0010; `gix_worktree_state`'s
full checkout API (the `worktree-mutation` feature) was evaluated and
deliberately not used — its `Options` require filesystem capability
probing, `gix-worktree`'s attributes stack, and a real `gix_filter`
pipeline, machinery built for clone-time/`git checkout --force` parity
with `.gitattributes`-driven text conversion, which is more than this
deliverable needs; `checkout_tree` is a focused materializer built from
primitives already used elsewhere in this crate (tree traversal from
`diff_panel`, index writing from `staging`).

**One real `gix` semantics finding, worth stating plainly:**
`PreviousValue::MustNotExist` on a ref *update* (not deletion) only
rejects the write if the existing ref's target actually differs from the
new value — re-running `repo.reference(name, same_target, MustNotExist,
...)` is treated as an idempotent no-op success, not an error, per
`gix-ref`'s own file-transaction source
(`store/file/transaction/prepare.rs`: `(MustNotExist, Some(existing)) =>
if existing.target != *new { Err(...) }`). `branches::create`'s test
suite originally asserted create-twice-with-the-same-target fails, and
that assertion was simply wrong about `gix`'s semantics, caught
immediately by the test actually running (not by reading docs first);
fixed by testing the real failure mode (creating an existing branch name
pointing at a *different* commit) and adding a second test that pins down
the idempotent-same-target case as intentional, documented behavior
rather than leaving it as a surprise. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-git branches::` (6 tests) and
`cargo test -p lux-git stash::` (5 tests), all against real temporary
repositories built with `gix`'s own object-writing API. Not yet built:
per-hunk or partial staging feeding into stash (stash always snapshots
every tracked file's current on-disk content, matching plain `git
stash`'s default — no `-u`/untracked-file support), conflict handling
when a checkout would overwrite unsaved changes, and — same as every row
in this section — the `lux-ui` wiring.

**A second real `gix` finding, caught by CI rather than local testing**:
`branches::create`/`switch`, `commit::amend`, and `stash::save`/`pop` all
write a ref update with a ref-log entry, and every one of them originally
relied on `gix`'s ambient `Repository::committer()` to supply that
ref-log's identity (via the `reference()`/`edit_reference()`
convenience methods). Per `gix`'s own documented design
(`repository/identity.rs`: "There is no notion of a default user like in
git, and instead failing to provide a user is fatal"), that call returns
`None` — a hard `MissingCommitter` error — unless `user.name`/`email` is
configured somewhere `gix` can see it, with **no** fallback to a guessed
`user@host` the way real `git` has for non-commit ref updates. Every
local run in this phase passed because the development machine's global
`~/.gitconfig` has an identity configured; the `macOS` GitHub Actions
runner has none, so every test exercising these four operations failed
there with `RefEdit(FileTransactionCommit(CreateOrUpdateRefLog(MissingCommitter)))`
the first time this branch's PR ran CI — a genuine portability bug, not
a CI quirk, since the same failure would hit any real user who has git
installed but hasn't set an identity yet. Fixed by threading explicit
`committer_name`/`committer_email` parameters through all four functions
(matching the pattern `commit`/`amend` already used) and calling `gix`'s
`edit_references_as(edits, Some(committer))` instead of the ambient-only
convenience methods. Verified both normally and with a from-scratch,
git-identity-free environment (`env -i HOME=<empty> GIT_CONFIG_NOSYSTEM=1
GIT_CONFIG_GLOBAL=/dev/null cargo test -p lux-git`): all 74 tests pass
either way, confirming the fix removes the dependency rather than just
happening to work on this machine.

**Scope note (`lux-git::merge_conflicts`, tracked in
`docs/receipts/phase-06-resolucion-conflictos-merge.md`)**: the "Merge
conflict resolution" row above doesn't move — same reasoning as every
scope-noted row in this document: a real, tested capability, nothing
wired into `lux-ui`'s editor (no "Accept Current / Incoming / Both"
commands exist yet — this loop builds the primitives such an action would
call, not the action itself). What exists and is verified: `merge()` — a
genuine three-way merge computed by `gix`'s own merge engine
(`Repository::merge_commits()`, automatic merge-base detection), which
materializes its result onto the working directory and index via the
same `checkout_tree` primitive branch-switching and stash use, so a
merge with conflicts leaves real `<<<<<<<`/`=======`/`>>>>>>>` markers in
the working tree files, exactly as real `git merge` does; `resolve()` —
given a conflict and an `Ours`/`Theirs`/`Both` choice, writes the chosen
content to the working file and stages it, the exact primitive an
"Accept Current / Incoming / Both" editor action would call per conflict;
and `complete()` — writes the two-parent merge commit from whatever's
currently staged once conflicts are resolved. No shell-out to `git`, per
ADR 0010 — and per that same ADR, `gix-merge`'s maturity was confirmed by
using it directly (same discipline that already paid off for blame
earlier in this phase) rather than assumed or worked around. **Scope is
deliberately narrower than everything `gix-merge` can report**: only
*content conflicts* (both sides modified, or both sides independently
added, the same path) are turned into a `MergeConflict` — rename
conflicts and delete/modify conflicts are real, detected outputs this
loop doesn't yet surface as resolvable; `merge()` reports those
explicitly via `MergeConflictError::UnsupportedConflictShape` rather than
silently dropping or misrepresenting them, so a caller always knows when
it hit a shape this module doesn't handle instead of getting a
subtly-wrong result. Verified on macOS (aarch64-apple-darwin): `cargo
test -p lux-git merge_conflicts::` — 7 tests, 0 failures, 0 ignored,
against a real temporary repository with an **induced** conflict (two
branches independently edit the same line of the same file from a shared
base) — not a synthetic/mocked conflict object: a merge with no real
conflicts leaves the non-conflicting change already merged on disk, the
induced conflict is detected with correct `ours`/`theirs` blob content
and real conflict markers written to the working copy, each of
`Ours`/`Theirs`/`Both` resolves correctly, and completing the merge
writes a real two-parent commit whose tree reflects the resolution
chosen. Not yet built: rename and delete/modify conflict resolution,
merge abort, and — same as every row in this section — the `lux-ui`
wiring (the "Accept Current / Incoming / Both" editor commands
themselves).

**Closing loop: `lux-git::status`, inline diff, per-hunk staging, and
verification against a real large repository** (tracked in
`docs/receipts/phase-06-completar-pendientes.md`). A `/close-phase 6`
audit found four real gaps against the master prompt's full entregable
list and Phase 6's own acceptance criteria, none covered by the six
receipts above: no repository-status summary existed at all; "diff lado
a lado **y en línea**" only had the side-by-side half; "stage... **y por
hunk**" only staged whole files; and — most importantly — nothing in
Phase 6 had actually been run against a real, large repository, which
the phase's acceptance criteria explicitly requires ("usa el propio
repo de Rust o de Linux como caso de prueba"). This loop closed the
first three and ran the fourth, which surfaced two real bugs neither
the small-repo test suite nor manual reasoning had caught:

- **`status`/`diff_panel::changed_files` bug**: a tracked file that also
  matches a `.gitignore` rule added *after* it was committed (a common
  real pattern — rust-lang/rust has 12 such files, e.g. `.vscode/
  settings.json` committed early, ignored later) was reported as
  **deleted**, because the gitignore-aware directory walk this crate
  uses to discover files on disk correctly skips it, and the old code
  treated "not found by the walk" as "gone." Real `git status` never
  lets `.gitignore` hide an already-tracked path — the rule only ever
  governs *un*tracked-file discovery. Fixed: every tracked path is now
  checked directly against disk, independent of the gitignore-aware
  walk, which is now used only to discover genuinely untracked files.
  Regression test: `diff_panel::tests::against_real_repo::
  a_tracked_file_that_matches_a_later_gitignore_rule_is_not_reported_as_deleted`.
- **`stash::save` bug**: a file that was `git add`-ed (staged) but never
  committed was invisible to `save` — it only scanned `HEAD`'s tree, not
  the index — so stashing right after staging a new file failed with
  "nothing to stash" despite a real staged change sitting there. Real
  `git stash` (even without `-u`) does capture staged-but-uncommitted
  files; only genuinely untracked-and-unstaged files are excluded by
  default. Fixed: the snapshot now covers the union of `HEAD`'s tree and
  the current index, not `HEAD`'s tree alone. Regression test:
  `stash::tests::save_includes_a_newly_staged_file_not_yet_committed`.

Both were caught specifically *because* the verification ran against a
repository with real, messy history (rust-lang/rust, shallow-cloned:
~62,000 files, a 71,164-line real source file) rather than only the
small, clean fixtures every other Phase 6 test builds by hand — neither
bug is reachable from a repo with a handful of files and no
pre-existing `.gitignore`/staging history. This is exactly the value
the phase's large-repo acceptance criterion is protecting against, and
it paid off on the first real run.

**One gap found and deliberately not fixed, per ADR 0010's "write up the
finding, don't shell out" instruction**: `blame::blame_by_commit` fails
on shallow clones (`git clone --depth N`) once its history walk needs to
cross the shallow boundary — `gix-blame` 0.16.0 reads the raw `parent`
field from the boundary commit's object bytes and tries to fetch it,
rather than consulting `.git/shallow` the way `git blame` itself does,
and fails with `NotFound` on an object that was never fetched by design.
Confirmed directly: `git cat-file -t <that oid>` also fails locally (the
object genuinely isn't present — this is `git`'s intended shallow-clone
behavior, not corruption), while the real `git blame` binary succeeds
because it treats the boundary commit as parentless. This is narrow in
practice — a normal (non-shallow) clone, the ordinary case for a
repository opened in an editor, has every object `blame` might need, so
this shouldn't affect real usage — but it's a genuine, reproducible
`gix` limitation worth tracking if `lux` ever needs to support opening
shallow clones directly (CI checkouts, some `git worktree` setups).

What else exists and is verified from this loop: `status::status`
(current branch — named, detached, or unborn — plus a dirty/clean flag
and changed-file count, reusing `diff_panel::changed_files` for the
count rather than a second change-detection path); `diff_view::
inline_diff` (a unified-diff view alongside the existing `side_by_side`,
both now living in their own module split out of `diff_panel` once
adding it pushed that file over the 600-line cap — pure rendering,
`diff_panel` keeps just the changes-tree data source); and `staging::
unstaged_hunks`/`stage_hunk` (per-hunk staging: diffs a path's currently
staged-or-`HEAD` content against its working-copy content via `gutter::
diff_hunks`, and stages one selected hunk by applying it onto that same
base — matching `git add -p`, verified with two independent edits to one
file staged one hunk at a time, each producing exactly the expected
partial content). Verified on macOS (aarch64-apple-darwin): `cargo test
-p lux-git` — 74 tests, 0 failures, 0 ignored, plus the full workspace
gate (`cargo test --workspace`, `cargo clippy --workspace -- -D
warnings`, `cargo deny check`, `scripts/check-limits.sh`,
`scripts/check-core-isolation.sh`) all clean — and, separately, the
manual large-repository verification pass described above (not part of
the automated suite — a 459 MB shallow clone isn't something to bundle
into routine `cargo test`, matching how every other loop's "verified
on..." line already describes a manual pass, not a promise that the
exact commands are checked into CI). Still not built, and not attempted
here because it is out of `lux-git`'s scope by golden rule 2, not an
oversight: the "Accept Current / Incoming / Both" commands themselves
(they live in the editor, which is `lux-ui`, and `lux-ui` doesn't
consume `lux-git` at all yet) and rename/delete-modify conflict
resolution (unchanged from the prior loop's note).

## LSP client (Phase 7, `v0.6.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Server lifecycle management | implemented | not-started | not-started | excluded (no child processes on iOS — ADR 0006) | not-started |
| Incremental document sync | implemented | not-started | not-started | excluded | not-started |
| Diagnostics panel | implemented | not-started | not-started | excluded | not-started |
| Autocomplete | implemented | not-started | not-started | excluded | not-started |
| Navigation (definition/references/symbols) | implemented | not-started | not-started | excluded | not-started |
| Hover / signature help / inlay hints | implemented | not-started | not-started | excluded | not-started |
| Code actions + `WorkspaceEdit` | implemented | not-started | not-started | excluded | not-started |
| Cross-file rename | implemented | not-started | not-started | excluded | not-started |
| Formatting | implemented | not-started | not-started | excluded | not-started |

**Scope note (`lux-lsp` server management — discovery, launch, capability
handshake, clean shutdown, crash restart, one server per language+root, and
visible per-server status with `$/progress`, tracked in
`docs/receipts/phase-07-gestion-servidores-arranque-capacidades-reinicio.md`)**:
the "Server lifecycle management" row above doesn't move yet — same
reasoning as every `not-started` row elsewhere in this document with a
scope note: `lux-lsp` is a headless client library with nothing wired into
`crates/lux-ui` or the running `lux` binary, so no user of the app can see
a language server's status yet. What exists and is verified: `ServerRegistry`
(one `ServerSession` per `(language_id, root)` pair, keyed after
canonicalizing the root, reusing a live session and replacing a
stopped/failed one), `discover_command` (a `PATH` search performed ahead of
spawning, so a missing binary reports a clear `Failed` status instead of an
opaque spawn error), and `ServerSession` itself: real `initialize`/
`initialized` handshake capturing the server's `ServerCapabilities`, clean
`shutdown`/`exit` with a kill fallback if the server doesn't exit within its
timeout, automatic restart (up to 3 attempts) when the connection drops
without a matching stop request, and a `watch`-channel `ServerState`
(status, capabilities, pid, restart count, and live `$/progress` — both
current in-flight operations and a monotonic event counter) that updates
event-driven, never polled. Built on `async-lsp` 0.2.4 (transport,
JSON-RPC routing, `LanguageServer`/`LanguageClient` omnitraits) and
`async-process` (child process I/O compatible with `async-lsp`'s
`AsyncRead`/`AsyncWrite` transport) — the final pick ADR 0011 deferred to
this loop; see that ADR for why `async-lsp` won over `lsp-server` and
`tower-lsp-server`. Verified on macOS (aarch64-apple-darwin) against a real
`rust-analyzer` (no fake/mock server): `cargo test -p lux-lsp session:: --test
lsp_lifecycle` (5 tests) — capability handshake (asserts a real
`hover_provider` capability), `$/progress` reporting during indexing, clean
shutdown, one-server-per-language-and-root reuse, and crash recovery (the
test sends a real `SIGKILL` to the server's OS pid and asserts a fresh pid
and a re-completed handshake afterward). Not yet built: everything else
Phase 7 asks for (diagnostics, completion, navigation, hover/signature
help/inlay hints/semantic tokens, code actions, cross-file rename,
formatting — document sync landed in the next loop, see the scope note
below) and the `lux-ui`/`lux` wiring that would move any row in the table
above.

**Scope note (`lux-lsp::sync` — incremental document synchronization,
tracked in `docs/receipts/phase-07-sincronizacion-incremental-documentos.md`)**:
the "Incremental document sync" row above doesn't move yet — same reasoning
as every `not-started` row elsewhere in this document with a scope note:
`lux-lsp` is still a headless client library, nothing wired into
`crates/lux-ui`. What exists and is verified: `DocumentSync`, tracking a
session's open documents and speaking `didOpen`/`didChange`/`didSave`/
`didClose` to it. `didChange` sends one content-change event per edit with
the caller's own range (`TextDocumentSyncKind::INCREMENTAL`) when the
server negotiated that — read from the real `ServerCapabilities` the
handshake captured, never assumed — and falls back to sending the whole
resulting document in a single event for `FULL`, or per-spec default when
the server didn't specify. Position math (`apply_edit`) resolves LSP
ranges as UTF-16 code units per line, not raw bytes, so a range past a
multi-byte character (e.g. past `é`) still splices at the right UTF-8
boundary. `didSave` includes the document text only when the server's
`TextDocumentSyncOptions::save::include_text` asked for it. A crash
restart (this crate's Phase 7 server-management loop) doesn't silently
break sync: `DocumentSync` watches its session's live connection and
automatically replays `didOpen` for every still-open document against the
fresh server process once it reconnects, since the new process has no
memory of the old one's state and no caller-side action re-triggers it.
Verified on macOS (aarch64-apple-darwin) against a real `rust-analyzer`:
`cargo test -p lux-lsp sync::` (11 tests) — 8 pure unit tests (edit-splicing
including the UTF-16 case, and sync/save capability-negotiation logic
against hand-built `ServerCapabilities` values, no process spawned) plus 3
tests against a real spawned server, each verified via a raw `hover`
request as an independent oracle rather than just asserting "no error":
an incremental append is genuinely visible to the server's own analysis
(hovering the newly-added function resolves it), save/close round-trip and
`NotOpen` after close, and — the crash-interaction test — a document
edited only in memory (never written to disk) is still recognized by hover
after a real `SIGKILL`-triggered restart, which only a working replay can
produce (the on-disk file alone doesn't contain that edit). Not yet built:
everything else Phase 7 asks for besides server management, document sync,
and diagnostics (see the next scope note), and the `lux-ui`/`lux` wiring
that would move any row above.

**Scope note (`lux-lsp::diagnostics` — continuous `publishDiagnostics` and
a filterable/navigable problems view, tracked in
`docs/receipts/phase-07-diagnosticos-panel-problemas.md`)**: the
"Diagnostics panel" row above doesn't move yet — same reasoning as every
`not-started` row elsewhere in this document with a scope note:
`lux-lsp` is still a headless client library, nothing wired into
`crates/lux-ui` (no actual panel, no editor underline). What exists and is
verified: reception is real and continuous — `session.rs`'s `Router` now
handles `textDocument/publishDiagnostics` (previously silently swallowed
by the catch-all handler), replacing the full diagnostic list for that
document on every notification per the spec (an empty list removes the
entry, not a stale empty one) and clearing all diagnostics on
crash/stop, since they're unknown until the reconnected server (or
`DocumentSync`'s replay) republishes them. `Diagnostics` is the query
layer on top: `all()`/`for_document()`, `filtered(min_severity)` (numeric
severity comparison, defaulting an *unmarked* severity to `ERROR` so a
filter can never accidentally hide it), `summary()` (per-severity counts
for a panel header or status bar), and `next_after`/`previous_before` for
diagnostic-to-diagnostic navigation in a stable (document, then position)
order that wraps around at either end. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-lsp diagnostics::` (8 tests) —
7 pure unit tests (sorting, severity filtering including the
unmarked-severity case, summary counts, and next/previous navigation with
wraparound, against hand-built diagnostic data, no process spawned) plus
one test against a real `rust-analyzer`: a syntax error is opened, its
real `ERROR`-severity diagnostic is observed through both the raw watch
channel and the `Diagnostics` query methods, then an incremental edit
(the previous deliverable's `didChange`) fixes it and the diagnostic is
confirmed to clear — proving diagnostics update continuously as the
document changes, not just once at `didOpen`. Not yet built: completion
(see the next scope note), navigation, hover/inlay hints/semantic tokens,
code actions, cross-file rename, formatting, and the `lux-ui`/`lux` wiring
(panel UI, gutter/editor underline, diagnostic navigation commands) that
would move any row above.

**Scope note (`lux-lsp::completion` — completion with `resolve`, tracked
in `docs/receipts/phase-07-autocompletado-resolve.md`)**: the
"Autocomplete" row above doesn't move yet — same reasoning as every
`not-started` row elsewhere in this document with a scope note: `lux-lsp`
is still a headless client library, nothing wired into `crates/lux-ui`
(no popup UI, no keystroke-triggered invocation). What exists and is
verified: `Completion::complete` requests `textDocument/completion` and
sorts the result the way the spec makes client sorting mandatory
(`sortText`, falling back to `label` when a server omits it — proven both
against hand-built data and against `rust-analyzer`'s real, ~200-item
`Vec<i32>` method-completion list); `Completion::resolve` requests
`completionItem/resolve` for server-provided documentation and detail,
returning the item unchanged when the server didn't advertise
`completionProvider.resolveProvider` rather than sending a request the
server never asked for. Getting real `resolve` support out of
rust-analyzer needed one more addition to `session.rs`'s `initialize`
handshake: declaring
`textDocument.completion.completionItem.resolveSupport` (which properties
we want lazily resolved) and `snippetSupport` in `ClientCapabilities` —
without it, rust-analyzer's own `completionProvider.resolveProvider`
capability came back `Some(false)`, confirmed by inspecting the real
capability payload before adding the declaration. Snippet items
(`insertTextFormat: Snippet`) are surfaced faithfully via the raw
`lsp_types::CompletionItem` the server sent — expanding snippet
placeholders in the editor is Phase 1's already-implemented `lux-core`
snippet engine's job once `lux-ui` wires the two together, not something
this deliverable reimplements. "Filtrado incremental sin latencia
perceptible" is handled as staleness rejection: each `complete()` call
tags itself with a monotonic generation counter, and a call whose
response arrives after a newer call has already started returns
`Err(CompletionError::Superseded)` instead of its (by then stale) result
— so a fast typist's screen can never have an older keystroke's results
flash in over a newer one's, regardless of real network response
ordering. Verified on macOS (aarch64-apple-darwin): `cargo test -p lux-lsp
completion::` (6 tests) — 3 pure unit tests (sort-by-`sortText`,
label-fallback, and a case mixing both within one list) and 3 tests
against a real `rust-analyzer`: real completions listed and confirmed
already in server-sort order end to end, `resolve` confirmed to fill in
real documentation text for `Vec::push` (not just "no error"), and the
staleness-rejection race itself, proven with a spawned in-flight request
that a fresher direct request supersedes. Not yet built: navigation (see
the next scope note), hover/inlay hints/semantic tokens, code actions,
cross-file rename, formatting, and the `lux-ui`/`lux` wiring that would
move any row above.

**Scope note (`lux-lsp::navigation` — definition, declaration, type
definition, implementations, references, call hierarchy, document and
workspace symbols, tracked in
`docs/receipts/phase-07-navegacion-definicion-referencias-simbolos.md`)**:
the "Navigation (definition/references/symbols)" row above doesn't move
yet — same reasoning as every `not-started` row elsewhere in this document
with a scope note: `lux-lsp` is still a headless client library, nothing
wired into `crates/lux-ui` (no jump-to-definition keybinding, no problems
list, no outline panel). What exists and is verified: `Navigation` wraps
all nine LSP navigation requests the master prompt asks for —
`definition`/`declaration`/`typeDefinition`/`implementation` (one shared
normalizer, since `lsp_types` defines the latter three as literal type
aliases of `GotoDefinitionResponse`, confirmed by reading the crate source
rather than assumed — collapsing `Scalar`/`Array`/`Link` variants into a
plain `Vec<Location>`, resolving a `LocationLink`'s
`target_selection_range` rather than its coarser `target_range`),
`references`, `documentSymbol`/`workspace/symbol` (both normalized into
one `SymbolEntry`/`WorkspaceSymbolEntry` shape regardless of whether the
server sent the modern nested form or the older flat
`SymbolInformation` list), and the three-request call hierarchy flow
(`prepareCallHierarchy` → `incomingCalls`/`outgoingCalls`). Verified on
macOS (aarch64-apple-darwin): `cargo test -p lux-lsp navigation::` (9
tests) — 6 pure unit tests (all four `GotoDefinitionResponse` variants,
nested-vs-flat document symbol normalization, no process spawned) and 3
tests against a real `rust-analyzer`, exercising a small but real fixture
(a struct, its constructor, and two functions with a genuine
caller/callee relationship) with actual semantic assertions, not just
"the request didn't error": `definition` on a call site resolves to the
real function definition line, `references` on a type finds multiple real
usages, `implementations` finds the real `impl` block, `typeDefinition`
resolves a variable back to its struct, `documentSymbol`/`workspace/symbol`
list the real struct/function/method names, and `prepareCallHierarchy` +
`incomingCalls`/`outgoingCalls` walk the real caller and the real callee.
Not yet built: hover/signature help/inlay hints/semantic tokens (see the
next scope note), code actions, cross-file rename, formatting, and the
`lux-ui`/`lux` wiring that would move any row above.

**Scope note (`lux-lsp::hover`/`signature`/`inlay_hints`/`semantic_tokens`
— hover, signature help, inlay hints, and semantic tokens, tracked in
`docs/receipts/phase-07-hover-firma-inlay-hints-semantic.md`)**: the
"Hover / signature help / inlay hints" row above doesn't move yet — same
reasoning as every `not-started` row elsewhere in this document with a
scope note: `lux-lsp` is still a headless client library, nothing wired
into `crates/lux-ui` (no hover popup, no signature tooltip, no inline
hint rendering, no tree-sitter-combined highlighting). What exists and is
verified: `Hover` flattens the server's three response shapes
(`MarkedString` scalar/array, `MarkupContent`) into one plain/markdown
text block; `Signature` requests `textDocument/signatureHelp` and
resolves the active signature/parameter, including
`ParameterLabel::LabelOffsets` (a UTF-16-code-unit slice into the
signature's own label — the same UTF-16-aware position handling the
document-sync deliverable's `apply_edit` needed) as well as the simpler
`Simple` string label; `InlayHints` requests `textDocument/inlayHint` for
a visible range and flattens each hint's label (a plain string, or an
array of interactive `InlayHintLabelPart`s) to plain text;
`SemanticTokensClient` decodes the server's delta-encoded token stream
(each token's line/column relative to the *previous* token, per spec)
into absolute `(line, start_char, length)` spans, with type/modifier
indices resolved to real names via the server's advertised legend,
falling back to `"unknown"` for an out-of-range index rather than
panicking. `session.rs`'s `Router` now also answers
`workspace/semanticTokens/refresh` and `workspace/inlayHint/refresh`
(server-to-client requests telling the client its cached data is stale)
with `Ok(())` rather than the default `METHOD_NOT_FOUND`. Verified on
macOS (aarch64-apple-darwin): `cargo test -p lux-lsp -- hover::
signature:: semantic_tokens:: inlay_hints::` (20 tests — note the `--`
before the filters: `cargo test` itself only accepts one positional
filter argument, so passing several requires forwarding them to the test
harness, which does support multiple) — 16 pure unit tests (all three
hover content shapes; active signature/parameter selection including the
UTF-16 offset case; both inlay-hint label shapes; the delta-decoding math
across same-line and new-line token deltas, plus legend resolution and
the out-of-range fallback) and 4 tests against a real `rust-analyzer`,
sharing the navigation deliverable's `Point`/`make`/`use_it` fixture:
hovering a call site shows the real function signature and return type,
signature help inside a real call shows the real parameter names
(`x: i32`/`y: i32`, confirming rust-analyzer's parameter labels include
type annotations, not just bare names), a real inferred-type inlay hint
(`Point`) is found on an unannotated `let` binding, and a full
semantic-tokens request is decoded into real, in-range, non-zero-length
spans with real `struct`/`function` type names resolved from the legend.
**Combining semantic tokens with tree-sitter's own highlighting** (the
master prompt's exact phrase for this deliverable) is a rendering-layer
decision — which span wins where the two disagree — that needs both
`lux-syntax` (Phase 3) and `lux-lsp` wired into `lux-ui`'s renderer; this
loop delivers the necessary piece before that's even possible (real,
decoded, legend-resolved spans), not the combining itself, consistent
with every other Phase 7 deliverable's headless-library boundary. Not yet
built: code actions (see the next scope note), cross-file rename,
formatting, and the `lux-ui`/`lux` wiring that would move any row above.

**Scope note (`lux-lsp::code_actions` — code actions and quick fixes
applying `WorkspaceEdit` transactionally, tracked in
`docs/receipts/phase-07-code-actions-workspaceedit-transaccional.md`)**:
the "Code actions + `WorkspaceEdit`" row above doesn't move yet — same
reasoning as every `not-started` row elsewhere in this document with a
scope note: `lux-lsp` is still a headless client library, nothing wired
into `crates/lux-ui` (no lightbulb menu, no quick-fix popup). What exists
and is verified: `CodeActions` requests `textDocument/codeAction` and
lazily resolves an action's `edit` via `codeAction/resolve` only when the
server advertised `resolveProvider` and the action doesn't already carry
one; `preview` normalizes any `WorkspaceEdit` shape (the legacy flat
`changes` map, the newer `document_changes` list of `TextDocumentEdit`s,
or the full `DocumentChangeOperation` mix including file create/rename/
delete) into one ordered, inspectable `Vec<FileChange>`, preferring
`document_changes` over `changes` per spec when a server sends both.
**Transactional apply, including multi-file edits**, is a new
`DocumentSync::apply_all` (extending the document-sync deliverable's
module, the same category of necessary cross-deliverable extension every
prior Phase 7 loop has needed once): it validates every targeted URI is
currently open *before* mutating anything, so a single not-open target
aborts the whole call with nothing changed, proven for real by a test
that edits two open documents together, then tries a second call mixing
one of those same documents with a third, never-opened one and confirms
neither open document changed. A `WorkspaceEdit` that also creates,
renames, or deletes a file is rejected outright (`Err`, nothing applied)
rather than silently applying just its text-edit portion, since this
client owns already-open in-memory documents, not the filesystem, and
partially honoring an edit would be less than what it actually asked
for. Getting a real `WorkspaceEdit` out of rust-analyzer at all needed
one more `session.rs` client-capability declaration: without
`textDocument.codeAction.codeActionLiteralSupport` (plus `resolveSupport`
and `dataSupport`), rust-analyzer's `textDocument/codeAction` returned an
empty list for every request, discovered by confirming the client's
capability payload was likely the gap (the same category of fix the
completion and hover/signature deliverables each hit once) rather than
concluding no actions were available. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-lsp -- code_actions::` (8
tests) — 6 pure unit tests (`WorkspaceEdit` normalization for all three
shapes, `document_changes` taking precedence over `changes`, resource
operations rejected with nothing converted, edits for the same file
merged) and 2 tests against a real `rust-analyzer`: a real "Extract into
variable" code action is requested, resolved, applied, and the open
document's text is confirmed to have actually changed (a real `let`
binding appears); and the multi-file transactional guarantee itself,
proven both in the success case (two open files edited together) and the
all-or-nothing case (a third unopened target aborts the whole call,
neither open file touched). Not yet built: cross-file rename (see the
next scope note), formatting, and the `lux-ui`/`lux` wiring that would
move any row above.

**Scope note (`lux-lsp::rename` — cross-file rename with multi-file
preview and per-hunk accept/reject, tracked in
`docs/receipts/phase-07-rename-crossfile-preview-multifichero.md`)**: the
"Cross-file rename" row above doesn't move yet — same reasoning as every
`not-started` row elsewhere in this document with a scope note: `lux-lsp`
is still a headless client library, nothing wired into `crates/lux-ui`
(no rename dialog, no diff preview panel). What exists and is verified:
`Rename::prepare` calls `textDocument/prepareRename` to validate the
target and get its real current name before a rename is even attempted —
the same "ask before assuming" real editors do; `Rename::rename` requests
`textDocument/rename` and returns the server's `WorkspaceEdit` unmodified
(never string replacement — the master prompt's explicit "nada de
reemplazo de cadenas"); `preview` reuses the code-actions deliverable's
`WorkspaceEdit` normalization and flattens every file's edits into
individually addressable `RenameHunk`s, each with a stable id. **Apply or
reject per hunk**, not just per file or all-or-nothing across the whole
edit: `Rename::apply` takes a set of accepted hunk ids and applies
exactly those, atomically, via the document-sync deliverable's
`apply_all` — rejected hunks are simply never sent, proven for real by a
test that accepts only one file's hunks from a two-file rename, confirms
the other file is untouched, then applies the remainder in a second,
independent call. A rename that also requests a file create/rename/
delete is refused outright, same reasoning as code actions: this client
owns already-open in-memory documents, not the filesystem. **A latent
multi-edit ordering bug in `DocumentSync`, found and fixed while building
this deliverable**: `change`/`apply_all` applied same-file edits in
array order via naive sequential folding, but LSP `TextEdit` ranges are
always relative to the *original* document — an earlier edit that shifts
line/column length (near-universal for a rename touching one file more
than once) silently invalidated every later edit's coordinates in that
same file. Neither prior deliverable's tests ever exercised multiple
edits in one file, so this went unnoticed until rename's real
multi-occurrence case exposed it. Fixed by sorting each file's edits to
descending document position before folding (and before building the
`didChange` sent to the server, in the same order, so both sides
converge on the same result) — a pure unit test (`let x = x + 1;`,
renaming both `x`s to the longer `index`) pins this down independent of
any real server. Verified on macOS (aarch64-apple-darwin): `cargo test -p
lux-lsp -- rename::` (7 tests) — 5 pure unit tests (hunk flattening with
stable ids, file operations kept separate from edit hunks, a file
operation rejecting the whole apply, only-accepted-ids included when
building the edit map) and 2 tests against a real `rust-analyzer`: a real
`prepareRename` response with the actual current name (`"Point"`) as the
placeholder, and a genuine cross-file rename (a struct defined in one
file, used via return type/associated-function call/struct literal in
both that file and a second one) previewed, applied to only one file's
hunks first (the other file confirmed completely unchanged), then the
remaining hunks applied in a second call, with both files finally showing
the renamed symbol and zero leftover occurrences of the old name. Not yet
built: formatting, and the `lux-ui`/`lux` wiring that would move any row
above.

**Scope note (`lux-lsp::formatting` — whole-document formatting, range
formatting, and format-on-save, with a per-language/per-workspace
formatter configuration, tracked in
`docs/receipts/phase-07-formateo-documento-rango-formatonsave.md`)**: the
"Formatting" row above doesn't move yet — same reasoning as every
`not-started` row elsewhere in this document with a scope note: `lux-lsp`
is still a headless client library, nothing wired into `crates/lux-ui` (no
"Format Document" command, no format-on-save preference UI). What exists
and is verified: `Formatting::document` requests
`textDocument/formatting`, `Formatting::range` requests
`textDocument/rangeFormatting`, and `Formatting::format_and_save` composes
the two prior deliverables' primitives — formats the whole document,
applies the result through `DocumentSync::change` when the server actually
returned edits, then calls `DocumentSync::save` — so what a real save
writes is the *formatted* text, not a pre-format snapshot with formatting
applied as an afterthought. `FormattingConfig`/`LanguageFormattingConfig`
give "formateador configurable por lenguaje y por workspace con precedencia
clara" a real precedence rule (`for_language` returns an explicit
per-language override when one exists, the workspace default otherwise),
not just a flat options struct. **A genuine upstream limitation, found and
handled honestly rather than forced or hidden**: rust-analyzer's real
`documentRangeFormattingProvider` capability came back `false` on this
machine's stable-toolchain rustfmt — the server's own error text confirms
why ("rustfmt range formatting is unstable ... opt-in ... using a nightly
build of rustfmt and setting `rustfmt.rangeFormatting.enable`"), a
rust-analyzer-specific nightly-only initialization option, not a general
LSP capability a client should assume or force on (that would tie this
client to one server's nightly config). The range-formatting test reads
the real advertised capability first and asserts accordingly: a server
that claims support must genuinely reformat only the requested range and
leave the rest byte-for-byte untouched; a server that doesn't (this
machine's real case) must still round-trip cleanly — empty edits or the
server's own clean error, never a panic or a hang. Document formatting and
format-and-save needed no such adaptation and worked end-to-end on the
first real-server run. Verified on macOS (aarch64-apple-darwin): `cargo
test -p lux-lsp -- formatting::` (6 tests) — 3 pure unit tests
(per-language override precedence, workspace-default fallback, sane
default options) and 3 tests against a real `rust-analyzer`: whole-document
formatting turns real unformatted `rustfmt`-style input into real
`rustfmt` output (verified spacing and indentation, not just "no error"),
the range-formatting capability round-trip described above, and
format-and-save confirmed to save the *formatted* text by retrying until
the document's own content actually changes and then reading it back.
Full crate suite re-run clean after this deliverable (`cargo test -p
lux-lsp`, 86 tests — 81 lib + 5 `lsp_lifecycle`, up from 80 lib after the
rename deliverable, confirming no regressions across all nine
deliverables). This closes out Phase 7's nine numbered deliverables;
the acceptance criteria beyond them (below), the `lux-ui`/`lux` wiring,
and the phase-level close-out are separate work.

**Closing update (fuzzer, `lux-lsp::framing`, tracked in
`docs/receipts/phase-07-fuzzer-parser-stream-jsonrpc.md`)**: the phase's
acceptance criteria ask for "un fuzzer del parser del stream JSON-RPC (v0
no tenía ninguno)," separate from the nine numbered deliverables above.
`crates/lux-lsp/fuzz/` (a standalone `cargo-fuzz` subcrate, same
isolation pattern as `lux-core/fuzz`) drives the real production path —
`lux_lsp::guard_stream` wrapping `async-lsp`'s own `Message::read` — with
raw fuzzer-generated bytes. It found two real bugs, both fixed in this
crate rather than narrowed around: (1) `async-lsp` 0.2.4's `Message::read`
allocates `vec![0u8; content_len]` straight from an attacker- or
bug-controlled `Content-Length` header with no upper bound, aborting the
whole client process on a single short, hostile header line — fixed by
the new `lux_lsp::guard_stream` (`src/framing.rs`), a transparent
pass-through that re-parses just the wire framing (never the JSON-RPC
payload itself, still entirely `async-lsp`'s job) and rejects any claimed
length over 64 MiB before an allocation is ever attempted, now wired
between every real child process's stdout and `async-lsp` in `session.rs`;
(2) re-fuzzing after that fix found a second bug in the guard itself — its
header-line matching was looser than `async-lsp`'s own, so a malformed
second header line could reset the guard's tracked length back to "none
found" while `async-lsp`'s stricter parser still read the real, huge value
from an earlier well-formed line and tried to allocate for it anyway;
fixed by matching `async-lsp`'s exact header-line syntax byte-for-byte, so
the two parsers can never disagree on what counts as a real
`Content-Length` line. Verified on macOS (aarch64-apple-darwin):
`cargo fuzz run lsp_stream_fuzz -- -max_total_time=1800` — **53,608,974
executions in 1801 seconds, zero crashes** — plus 8 pure unit tests in
`framing.rs` (including regression tests for both findings above).

**Closing update (acceptance criteria beyond the nine deliverables —
five real servers, a fake-server protocol suite, and a no-thread-blocking
proxy check, tracked in
`docs/receipts/phase-07-criterios-aceptacion-multiservidor-mentira-nobloqueo.md`)**:
every row in the table above still stays `not-started` — none of this
adds `lux-ui` wiring either — but it closes the remaining gaps
`/close-phase 7`'s own audit found: every one of the nine deliverables'
tests, without exception, had only ever run against `rust-analyzer`.
`crates/lux-lsp/tests/multi_server.rs` now runs the same real
spawn → handshake → `didOpen` → real `documentSymbol` sequence against
all five required servers (`rust-analyzer`, `typescript-language-server`,
`pyright`, `gopls`, `clangd`), and found a real client bug while doing
it: `session.rs`'s `initialize` call only ever sent `workspace_folders`,
never `root_uri` — spec-deprecated, but `typescript-language-server`
5.3.0 still needs it in practice to resolve its own `typescript`
dependency, confirmed by manually driving the server's raw stdio to
isolate the client's exact contribution before touching any code.
`session.rs` now sends both. Separately, `crates/lux-lsp/tests/
fake_server_protocol.rs` drives a new, genuinely real subprocess
(`lux-lsp-fake-server`, a test-only binary speaking the real wire
protocol with scripted responses) — the "servidor de mentira que ejerza
el protocolo" the criteria name as its own thing, distinct from the
fuzzer (malformed bytes) — proving three things a real server can't:
exactly what `InitializeParams` this client actually sent (a real server
never reports that back), an exact `$/progress` count instead of
whatever a real server happens to emit, and that a server exiting on its
own (not `SIGKILL`ed) triggers the same restart logic deliverable 1
proved against a signal. And `crates/lux-lsp/tests/no_thread_blocking.rs`
verifies the mechanism the literal "no LSP operation blocks the UI
thread, verified with the frame-time monitor" criterion depends on: a
slow, in-flight hover request never stalls a concurrent tick loop sharing
its executor thread — the strongest honest proxy for that criterion
until `lux-ui` exists to measure the real thing. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-lsp --test multi_server
--test fake_server_protocol --test no_thread_blocking` (9 tests), full
crate suite re-run clean after the `root_uri` and `ServerSession::spawn`
visibility changes.

## Terminal (Phase 8, `v0.7.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Real PTY terminal | implemented | not-started | not-started | excluded (no child processes on iOS — ADR 0006) | not-started |
| ConPTY | n/a | not-started | n/a | n/a | n/a |
| Scrollback, search, selection | implemented | not-started | not-started | excluded | not-started |
| Multiple named terminals / shell profiles | implemented | not-started | not-started | excluded | not-started |
| Build-error link detection | implemented | not-started | not-started | excluded | not-started |
| Task panel with cancellation | implemented | not-started | not-started | excluded | not-started |

**Scope note (`lux-term` PTY real — `portable-pty` + `alacritty_terminal`,
signals, resize; tracked in
`docs/receipts/phase-08-pty-real-portablepty-alacrittyterminal-senales.md`)**:
the "Real PTY terminal" row above doesn't move yet — same reasoning as
every `not-started` row elsewhere in this document with a scope note:
`lux-term` is a headless crate with nothing wired into `crates/lux-ui` or
the running `lux` binary, so no user of the app can open a terminal panel
yet. What exists and is verified: `Pty::spawn` opens a real OS pty via
`portable-pty`'s `NativePtySystem` and spawns a real, persistent child
process into it — not a one-shot captured command, the exact failure
golden rule #1 and ADR 0012 exist to forbid. Every byte the child writes
is fed through `alacritty_terminal`'s own VT parser (`vte::ansi::Processor`
driving a `Term`) on a dedicated reader thread that blocks on the real pty
file descriptor — event-driven, no polling loop (golden rule #5).
`Pty::resize` updates both the kernel's `TIOCSWINSZ` winsize and the VT
grid's dimensions together. `Pty::kill_and_wait` reaps the child so no
zombie or orphan is left behind. Signals need no dedicated API:
`Pty::write` sends raw bytes, and the kernel's own tty line discipline
turns control bytes like `0x03` into real `SIGINT` — verified with a real
`cat` process actually dying from a real `SIGINT`, not a simulated one.
`crates/lux-term/tests/lanza_vim_real.rs` drives a real `vim` end to end:
spawn, wait for its real intro screen to render, resize live, confirm
`vim` itself (not just the local grid) observed the new size via
`&columns`/`&lines` — proof `SIGWINCH` was actually delivered — send
`:q!`, and a clean exit with the child reaped. Scrollback, search,
selection, multiple named terminals/shell profiles, link detection, and
the task panel are separate, later deliverables in the same phase; Linux
is unverified — `portable-pty` claims to support it, but only macOS
(aarch64-apple-darwin) has actually been run there. ConPTY (Windows) is
covered separately below, now verified. Verified on macOS
(aarch64-apple-darwin): `cargo test -p lux-term` (4 unit tests +
`lanza_vim_real`, all passing, none ignored), `cargo clippy -p lux-term
--all-targets -- -D warnings` (clean), `cargo fmt -p lux-term --
--check` (clean).

**Scope note (`lux-term` ConPTY on Windows; tracked in
`docs/receipts/phase-08-conpty-windows.md`)**: the "ConPTY" row above
doesn't move yet, same not-wired-into-`lux-ui` reasoning as every other
scope note here — but unlike the notes for macOS-only deliverables,
this one is verified on the actual target platform, not just reasoned
about from source. `portable-pty`'s `NativePtySystem` already resolves
to its own `win::conpty::ConPtySystem` under `#[cfg(windows)]`, so no
new Windows-specific *spawn* code was needed — but a real, universal
terminal-correctness gap had to be fixed first: Windows' ConPTY sends
`ESC[6n` (a cursor position query) as part of its own startup handshake
and blocks all further output until it receives a reply, and `Pty` only
ever read bytes, never answered terminal queries. `alacritty_terminal`
already computes the correct reply (`Term::device_status` emits
`Event::PtyWrite`); `Pty::spawn`'s reader thread now drains that event
and writes real replies back to the pty, the same mechanism a real
terminal emulator (this crate's own dependency, and wezterm, which
`portable-pty` was extracted from) relies on — not the hardcoded
`ESC[1;1R` workaround several other, non-VT-emulating projects needed
for the identical symptom. `crates/lux-term/tests/conpty.rs`
(`#[cfg(windows)]`) pins `cmd.exe` spawn + live resize via a real
`ResizePseudoConsole` call, and kill + reap with no hang and no orphan
(`portable-pty`'s Windows `ChildKiller` calls `TerminateProcess`
directly, no `SIGHUP` grace period the way Unix does). Verified on
Windows via GitHub Actions' `windows-latest` runner (a real Windows
Server image with a real MSVC toolchain, not a cross-compile): all three
`conpty::` tests pass, run
`https://github.com/lcajigasm/lux/actions/runs/32119332445`. Getting
here took nine iterations, documented in full in the receipt, including
a real ConPTY-specific deadlock (`portable-pty`'s Windows master and
slave share one `Arc<Mutex<Inner>>`, so `Pty`'s Unix-derived "drop the
slave at spawn" pattern didn't release the pseudoconsole on its own) and
a still-open upstream `portable-pty` 0.9.0 issue
(wezterm/wezterm#6783) that a plain web search — not another blind CI
round — is what actually surfaced the fix.

**Scope note (`lux-term` scrollback, search, selection/copy; tracked in
`docs/receipts/phase-08-scrollback-busqueda-seleccion-copia.md`)**: the
"Scrollback, search, selection" row above doesn't move yet, same
not-wired-into-`lux-ui` reasoning as every other scope note here. What
exists and is verified, all as a thin `Pty`-shaped surface over
`alacritty_terminal` capabilities already present since the PTY-real
deliverable (`Config::scrolling_history` already keeps 10,000 lines of
history; nothing new needed there) — this deliverable didn't reimplement
any of it, only exposed it: `Pty::scroll` (`Scroll::{Lines,PageUp,
PageDown,Top,Bottom}`) moves the viewport into real scrollback via
`Term::scroll_display`; `Pty::scrollback_position` reports how much
history exists and how far the viewport is scrolled into it;
`Pty::search` runs `alacritty_terminal`'s real DFA-compiled regex search
(`RegexSearch`/`Term::search_next`) across the *entire* buffer, not just
the visible screen, and scrolls a match into view; `Pty::select` and
`Pty::clear_selection` wrap `alacritty_terminal`'s own `Selection` and
`Term::selection_to_string` for real select-and-copy. `Pty::snapshot`
(from the PTY-real deliverable) was fixed to render whatever the
viewport is currently scrolled to instead of always the live bottom —
a real bug the scrollback work surfaced, not a new feature. Verified on
macOS (aarch64-apple-darwin) with real, non-interactive shells (no
fixtures or canned VT sequences): a shell that outputs 100 lines into a
24-row viewport, scrolled to `Top` to confirm `line-1` becomes visible
and `Bottom` to confirm it returns to `line-100`, live; a regex search
for a marker line already scrolled out of the live viewport, found in
real scrollback and scrolled back into view; a selection over known
output text returning the exact expected string. `cargo test -p lux-term
scrollback::` (3 tests, re-run 4 times clean), full crate suite `cargo
test -p lux-term` (7 unit tests + `lanza_vim_real`, all passing, none
ignored — no regressions in the PTY-real or ConPTY deliverables), `cargo
clippy -p lux-term --all-targets -- -D warnings` (clean), `cargo fmt -p
lux-term -- --check` (clean), `./scripts/check-limits.sh` (clean —
`scrollback.rs` split out as its own file specifically to keep `pty.rs`
from growing indefinitely as more Phase 8 deliverables land on it).
Selection/copy and search are exposed as data (matched `Point` ranges,
extracted `String`s) with no UI attached — highlighting a selection or
rendering a find bar is `lux-ui`'s job, out of this headless crate's
scope. Named multiple terminals/shell profiles, link detection, and the
task panel are covered in the notes below.

**Scope note (`lux-term` named terminals + shell profiles; tracked in
`docs/receipts/phase-08-terminales-nombrados-perfiles-shell.md`)**: row stays
`not-started` until wired into `lux-ui`. `Terminals` + `ShellProfile` spawn
independent real PTYs; killing one session leaves the other live (verified
with `kill(pid, 0)`). `cargo test -p lux-term sessions::` clean on macOS.

**Scope note (`lux-term` build-error links; tracked in
`docs/receipts/phase-08-deteccion-enlaces-file-line.md`)**: `detect_links` /
`Pty::detect_links` return `SourceLocation` (path, line, optional column)
from rustc/gcc/Python shapes. No editor jump. `cargo test -p lux-term
links::` clean.

**Scope note (`lux-term` task panel; tracked in
`docs/receipts/phase-08-panel-tareas-cancelacion.md`)**: `TaskPanel::cancel`
is `Pty::kill_and_wait` — the child is gone (`ESRCH`). `cargo test -p
lux-term tasks::` clean.

**Scope note (Phase 8 close; tracked in
`docs/receipts/phase-08-cierre-vt-throughput-orphans.md` and
`docs/receipts/phase-08-release-notes.md`)**: VT pins + snapshot-after-flood
< 16 ms + parser rate gate live in `vt.rs` (test-only). Orphans: existing
`kill_and_wait` tests. iOS/iPadOS: `excluded` in the table above (ADR 0006);
crate rustdoc states the same. Android stays `not-started` (not in v0.7.0).
`cargo test -p lux-term` (19 unit tests + `lanza_vim_real`) clean on macOS.

## Plugins (Phase 9, `v0.8.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| `wasmtime` plugin host | implemented | not-started | not-started | not-started (subject to App Review — ADR 0006) | not-started |
| Capability-scoped permissions | implemented | not-started | not-started | not-started | not-started |
| Sandboxed CPU/memory limits | implemented | not-started | not-started | not-started | not-started |
| Extension points (commands, grammars, themes...) | implemented | not-started | not-started | not-started | not-started |
| Local plugin install | implemented | not-started | not-started | not-started | not-started |

**Scope note (`lux-plugin`; tracked in
`docs/receipts/phase-09-wasmtime-host-sandbox-permisos.md`, ADR 0016)**:
Wired into the `lux` desktop binary (`crates/lux/src/plugin.rs`, `crates/lux/src/render/plugin.rs`).
Component Model execution with fuel quota and memory limits, explicit capability permission grant
modal before running, sample plugins (`echo`, `theme-lux-dark-plus`, `identity-fmt`) registered
and executable via the Command Palette. `cargo test -p lux-plugin` (10 tests) and `cargo test -p lux plugin::` clean.

## Accessibility, i18n & polish (Phase 10, `v0.9.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Full keyboard navigation | implemented | not-started | not-started | not-started | not-started |
| Screen reader support | implemented (VoiceOver) | not-started (NVDA) | not-started (Orca) | not-started | not-started (TalkBack) |
| High contrast / UI scaling / reduced motion | implemented | not-started | not-started | not-started | not-started |
| i18n (es/en) | implemented | not-started | not-started | not-started | not-started |
| RTL layout | implemented | not-started | not-started | not-started | not-started |

**Scope note (Phase 10 chrome; tracked in
`docs/receipts/phase-10-accesskit-tree-validation.md` and `docs/receipts/phase-10-a11y-i18n-chrome.md`)**:
`lux` implements full keyboard navigation via `FocusRing` (`F6`/`Shift+F6`, `Escape` untrap),
full AccessKit window tree with live status announcer (`ANNOUNCER_ID`), 100% catalog key completeness
for English and Spanish in `lux-ui::i18n` with dynamic palette language switching (`view.locale_en`, `view.locale_es`),
and bidirectional text layout support (`lux-ui::rtl`). VoiceOver accessibility was validated on macOS
in an earlier pass. v1.2.0 adds an automated AccessKit tree check in the Linux CI
job (see the v1.2.0 note at the top). NVDA and Orca have not been run, so the Windows
and Linux screen-reader rows stay `not-started`.

## Packaging & distribution (Phase 11, `v1.0.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Signed, notarized installer | not-started (.dmg) | not-started (.msi + winget) | not-started (.deb/.rpm/AppImage/Flatpak) | n/a (App Store, Phase 12) | n/a (Play Store, Phase 13) |
| Auto-update (Ed25519 verify) | implemented | not-started | not-started | n/a | n/a |
| Local crash reports (no auto-send) | implemented | not-started | not-started | not-started | not-started |

**Scope note (Phase 11; `docs/receipts/phase-11-pack-crash-update.md`)**:
`lux-pack` writes crash logs under `~/Library/Logs/Lux` (macOS) and
verifies Ed25519 update payloads (`crates/lux-pack/src/update.rs`). `lux` installs the panic hook at startup
(`crates/lux/src/main.rs`). `scripts/package.sh` builds the release distribution tree (`dist/lux` containing
`bin/lux`, `share/doc/README.md`, `share/doc/LICENSE*`). Windows Package Manager manifest prepared in
`packaging/winget/Lux.Lux.yaml`. The tag workflow on GitHub builds the unsigned Linux
`.deb`. Publishing that release starts `release-macos.yml` for the universal
ad-hoc `.dmg`. `scripts/package-msi.ps1` still builds an unsigned `.msi` outside
CI (see the note at the top). Signed and
notarized installers stay `not-started`: no certificate is stored in the repo.

## Mobile shells (Phase 12 `v1.1.0`, Phase 13 `v1.2.0`)

| Capability | iPadOS/iOS | Android |
|---|---|---|
| Native platform-trait backend (UIKit / Kotlin+JNI) | not-started | not-started |
| Mobile text input (IME, selection handles, system menu) | not-started | not-started |
| External keyboard / trackpad / Stage Manager (iPad) | not-started | n/a |
| Sandboxed file access (`UIDocumentPicker` / SAF) | not-started | not-started |
| `accesskit_ios` / `accesskit_android` | not-started | not-started |
| App Store / Play Store signed build | not-started | not-started |

**Scope note (Phase 12/13; receipts `phase-12-uikit-host.md`,
`phase-13-android-host.md`)**: Swift `LuxViewController` (MTKView,
UIKeyInput, document picker) and Kotlin `LuxActivity` (SurfaceView,
InputConnection, SAF) live in `host-ios/` and `host-android/`. Rust FFI
modules compile on desktop as `available() == false`. Not run on device;
store rows stay `not-started`. Workspace version 1.5.0 is the desktop installer
release; these phase labels are the original plan names and are still not done.
`lux_platform::child_processes_allowed` is false on iOS.

## Experimental

_Nothing is behind an `experimental.*` flag yet._

---

## Post-Audit Hardening & Fix Verification (2026-10-15)

### ST-FIX-01: Modal Hit-Testing & Mouse Interaction Parity
- **Status:** Complete & verified across macOS desktop target.
- **Components:** `crates/lux/src/modal/` (`mod.rs`, `palette_goto.rs`, `pickers.rs`, `search.rs`, `tests.rs`), `crates/lux/src/chrome.rs`, `crates/lux/src/app.rs`.
- **Verification Evidence:**
  - `handle_modal_click` intercepts all left-clicks before any desktop chrome or editor canvas when any modal is active.
  - Clicking any result row in Command Palette (`Cmd+Shift+P`), Goto Anything (`Cmd+P`), Language Picker, Theme Picker, Snippet Picker, Build Picker, Project Picker, or Project Search immediately executes that item.
  - Hovering over modal rows (`handle_modal_hover` on `PlatformEvent::MouseMoved`) updates `selected_index` with instant visual highlighting and triggers live document previews (Goto Anything) or color scheme previews (Theme Picker).
  - Outside clicks dismiss active modals cleanly (restoring snapshots or original theme) and consume the click event (`return true`), preventing click-through into the text editor or moving cursor positions.
  - Clicking inside modal header / input text fields keeps the modal open and focuses input fields.
  - Automated tests: `crates/lux/src/modal/tests.rs` (14/14 tests pass), workspace tests (`cargo test --workspace` 233/233 pass).
  - Architecture checks: `scripts/check-limits.sh` passes (< 600 lines/file, < 25 fields/struct), `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` pass with 0 warnings.

### ST-FIX-02: Event Loop Wakeup, Native Folder Picker, Cmd+B Build Runner Exclusivity & About Lux Modal
- **Status:** Complete & verified across macOS desktop target.
- **Components:** `crates/lux/src/menu/queue.rs`, `crates/lux/src/menu/mod.rs`, `crates/lux/src/menu/macos.rs`, `crates/lux/src/menu/macos_view_menus.rs`, `crates/lux/src/about.rs`, `crates/lux/src/dialog.rs`, `crates/lux/src/render/about.rs`, `crates/lux/src/render/modals.rs`, `crates/lux/src/modal/mod.rs`, `crates/lux/src/app.rs`, `crates/lux/src/app/views.rs`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/palette/registry.rs`.
- **Verification Evidence:**
  - **Immediate Event-Loop Wakeup:** In `crates/lux/src/menu/queue.rs`, `post_action` invokes `wake()`, notifying `winit`'s event loop via the registered waker (`PlatformContext::background_waker`). Any menu item clicked via macOS Cocoa AppKit triggers immediate processing and redraw with zero lag, without needing subsequent mouse motion. Verified in `test_waker_invocation_on_post_action`.
  - **Native Folder Picker:** "File -> Open Folder..." and Command Palette `file.open_folder` invoke `app.open_folder_dialog(ctx)`, connecting `ctx.file_dialogs().pick_folder()` (`NSOpenPanel` via `rfd`) to select real directories and populate the workspace sidebar. Verified in `test_handle_menu_action_open_folder`.
  - **Cmd+B Shortcut Conflict Resolution:** Removed conflicting `b` shortcut from "Toggle Side Bar" in `crates/lux/src/menu/macos_view_menus.rs`. Cocoa routes `Cmd+B` exclusively to "Tools -> Build" (`MenuAction::Build`), while Toggle Side Bar cleanly uses `Cmd+K, Cmd+B` in the Sublime keymap.
  - **Real "About Lux" Modal Dialog:** "Lux -> About Lux" (`MenuAction::About`) and Command Palette `help.about` open a real styled card dialog showing app version `v1.0.0 (macOS)`, engine tagline, GPU acceleration details, MIT license, copyright, and an interactive "Close" button. Fully wired into modal hit-testing (`is_any_modal_open`, outside-click dismissal, Escape key dismissal, hover highlights).
  - **Strict Limit Compliance:** Grouped modal dialog state (`conflict_prompt` and `about`) under `DialogState` in `crates/lux/src/dialog.rs`, keeping `UiOverlays` at exactly 25 fields and all files < 600 lines.
  - **Automated tests:** 240/240 tests pass in `cargo test -p lux`.
  - **Tooling passes:** `scripts/check-limits.sh`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check` all pass cleanly with 0 warnings.

### ST-FIX-03: Zero-Copy Incremental Syntax Highlighting & Non-Blocking Edit Pipeline
- **Status:** Complete & verified across macOS desktop target.
- **Components:** `crates/lux-syntax/` (`lib.rs`, `Cargo.toml`), `crates/lux-core/` (`buffer/mod.rs`), `crates/lux/src/app/` (`syntax.rs`, `types.rs`), `crates/lux/src/app.rs`, `crates/lux/src/render/text.rs`, `crates/lux/src/input/` (`syntax_typing_tests.rs`, `dispatch.rs`, `mod.rs`), `crates/lux/src/document/save_ops.rs`, `crates/lux/src/search.rs`, `crates/lux/src/snippets/dispatch.rs`, `crates/lux/src/menu/mod.rs`, `crates/lux/src/palette/commands.rs`.
- **Verification Evidence:**
  - **Elimination of UI Buffer Clones:** Completely removed `App::sync_syntax_on_edit` and the `source: String` buffer clone from `SyntaxState`. Zero calls to `buffer.text().clone()` occur anywhere in the critical typing loop (`insert_at_cursor`, backspace, delete, snippets, search replace).
  - **Incremental Tree-sitter Edits:** `compute_input_edit` calculates `tree_sitter::InputEdit` directly from `lux_core::Edit` in $O(\log N)$ time using `ropey::Rope` byte/char/line metrics, without allocating or converting the document to `String`.
  - **Non-Blocking Background Worker:** `BackgroundHighlighter` uses unbounded `crossbeam_channel` channels with job coalescing and `EventLoopProxy` waker notifications. The worker thread parses directly against chunks of `Rope` via `parser.parse_with_options` with custom chunk callbacks, never materializing the whole file in memory.
  - **Zero-Latency Visual Shifting:** `shift_tokens` updates cached token byte offsets synchronously on the UI thread in nanoseconds ($O(T)$ where $T$ is token count) on every edit, preventing highlighting jumps while the background worker recalculates syntax trees.
  - **Sub-Millisecond Token Slicing in Render Pipeline:** Render loop (`render/text.rs`) no longer runs synchronous Tree-sitter queries on visible lines every frame; it queries precomputed tokens with zero allocations via binary search (`partition_point`).
  - **Continuous Typing Latency < 8ms:** Verified in `test_typing_latency_under_budget_in_large_document` on a 20,000-line synthetic document (~800 KB). 100 consecutive typing insertions execute in under 5 ms total (< 0.05 ms per keystroke), vastly outperforming the 8 ms (120 FPS) budget.
  - **Automated Tests:** 241/241 workspace tests pass (`cargo test --workspace`), including all syntax golden token tests.
  - **Tooling Passes:** `scripts/check-limits.sh` (< 600 lines/file, <= 25 fields/struct), `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check` all pass cleanly with 0 warnings.

### ST-FIX-04: Strict Tab Switch Isolation & Chord Priority Delegation
- **Status:** Complete & verified across macOS desktop target.
- **Components:** `crates/lux/src/document.rs`, `crates/lux/src/app.rs`, `crates/lux/src/input/dispatch.rs`, `crates/lux-config/keymaps/sublime.jsonc`, `crates/lux/src/document/shortcut_tests.rs`.
- **Verification Evidence:**
  - **Strict Modifier Exclusivity for Tab Switching:** Refactored `document_key` to mandate that switching tabs `1..9` requires strict modifier exclusivity (`Cmd` active, `Alt == false`, `Control == false`, `Shift == false` on macOS; `Control` active, `Alt == false`, `Meta == false`, `Shift == false` on Linux/Windows).
  - **Conflict-Free Split Layout Switching:** Combinations like `Cmd+Option+1..5` (e.g. `Cmd+Option+2` for 2 columns) and `Cmd+Shift+...` are no longer hijacked by tab switching. They pass through unhindered to `handle_keyboard_event` and `dispatch_keymap_or_chord` to switch layout presets while keeping the active tab untouched.
  - **Prioritized Chord Delegation:** When `app.ui.pending_chord.is_some()`, `document_key` immediately delegates event dispatch to `crate::input::dispatch::dispatch_keymap_or_chord`. Two-key chords such as `Cmd+K, Cmd+1` (`fold_all`), `Cmd+K, Cmd+2..5` (`fold_level_2..5`), and `Cmd+K, Cmd+B` (`toggle_sidebar`) execute their commands with top priority without switching tabs.
  - **Second-Key Modifier Flexibility:** Added alias definitions in `sublime.jsonc` allowing chords with or without repeating the modifier on the second key (`ctrl+k 1..5`, `ctrl+k j`, `cmd+k 1..5`, `cmd+k j`).
  - **Comprehensive Automated Tests:** Verified in `crates/lux/src/document/shortcut_tests.rs` (5 integration test scenarios for split layout preserving tab, `Cmd+K, Cmd+1` folding without altering tab, chord aliases, Escape cancellation, and strict modifier isolation).
  - **Workspace & Tooling Passes:** 247/247 tests pass in `cargo test -p lux`, `cargo test --workspace` passes cleanly, `scripts/check-limits.sh` passes (< 600 lines/file, <= 25 fields/struct), `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` pass with 0 warnings.

### ST-FIX-05: Modal Keyboard and Language Picker Scroll Ownership
- **Status:** Implemented and covered by the desktop event-path tests.
- **Components:** `crates/lux/src/app.rs`, `crates/lux/src/language_picker.rs`, `crates/lux/src/palette.rs`, `crates/lux/src/desktop_pass.rs`.
- **Verification Evidence:**
  - **Single insertion per physical key press:** the application discards key-release events before routing them to text fields. Picker and search handlers can no longer append the same printable key on both press and release.
  - **Modal-first input routing:** floating/focused surfaces receive keyboard input before document commands and editor insertion. Unhandled command chords still fall through to the configured keymap, while modal text cannot leak into the document.
  - **Literal New File shortcut:** `Cmd/Ctrl+N` always opens a separate untitled tab, including when the current tab is still empty; only opening the first real file may reuse the initial placeholder tab.
  - **Language picker wheel ownership:** line-wheel movement changes the selected language row; precise trackpad movement accumulates by row height. While the picker is open, the underlying editor viewport remains unchanged.
  - **Focus restoration:** dismissing or executing the Command Palette returns focus to the editor unless the selected command opens another focused surface.
  - **Automated tests:** the four `desktop_pass` tests pass, including press/release deduplication, modal isolation, language-list scrolling, `Cmd/Ctrl+Shift+P`, and literal `Cmd/Ctrl+N` tab creation through the real keyboard pipeline. Sidebar delete (`sidebar::tests::sidebar_duplicate_and_delete_operations`) moves the file with `NSFileManager.trashItemAtURL` on macOS. The earlier failure was `trash`'s default Finder AppleScript, which returns an error when the process has no Automation permission; the file still goes to the real Trash. `cargo clippy -p lux -- -D warnings`, `cargo fmt --check`, and `scripts/check-limits.sh` pass.

### ST-FIX-14: File tree refresh from the watcher

- **Status:** A file or folder created under a directory the sidebar has already read appears in that directory, including when the directory is collapsed. Removing it drops the row and any loaded children. A create inside a folder that has never been expanded waits for the next expand, which reads that directory. `.git` and the other hidden version-control directories stay out. The root row is not removed. The macOS capture writes `arrived.txt` beside an open folder and the sidebar lists it before the file is deleted again.
- **Verification:** `file_tree::tests::a_created_file_joins_a_loaded_directory_and_a_removed_file_leaves`, `a_created_file_in_a_collapsed_loaded_folder_shows_on_expand`, `a_file_inside_an_unloaded_folder_waits_for_expand`, `removing_a_directory_drops_its_loaded_children`, and `a_canonical_create_lands_in_a_tree_opened_through_a_symlink`. `sidebar::watch::tests::a_created_file_appears_and_a_removed_file_disappears` uses the real watcher. `scripts/tree-watch.sh` is the macos-15 capture.

### ST-FIX-13: Reload, hot exit, comments, scroll, and menu chords

- **Status:** An external edit of a clean open file reloads that buffer. The same edit of a dirty file opens the conflict dialog and leaves the buffer alone. The watcher wakes the editor loop from the debouncer thread. Closing with a dirty named file and another tab restores both, and the named tab comes back as the draft rather than the bytes on disk. `lux .` and a directory argument open that folder. The macOS bundle lists `public.folder`, so Finder can pass the same path. A file dropped on the window is a platform event and opens through the document worker. A second dropped file waits in that queue until the first worker finishes. A dropped directory is reported as an open error and leaves the current buffer alone. Command+/ comments every built-in language that has a comment, including an OCaml block comment. Diff and Regex have none. The native menu key letters for save, undo, redo, comment, duplicate, delete line, join, add next, find, and preferences are the Sublime keymap chords. The menu action and the keymap command share one claim, so the pair runs once. Trackpad pixel deltas, including a short momentum tail, move both axes by those pixels. A wheel over the sidebar scrolls the tree. Cold start of the release binary is process start until the first presented frame, median of ten launches, in `scripts/h1-cold-start.sh`. The median is the macOS artifact. A missing sample or a missing frame fails the job. A budget miss does not. Key-to-pixel and a 10 MB open are still not copied into this file. The side-by-side compare frames are typed; the scale-2 palette frame is still a renderer snapshot because the macOS runner framebuffer is 1×.
- **Verification:** `lux_workspace::watcher::tests::an_external_edit_invokes_the_registered_waker` and `session::tests::an_external_edit_reloads_a_clean_file_and_asks_when_dirty`. `session::tests::hot_exit_restores_a_dirty_named_file_beside_a_clean_tab`. `main::tests::dot_opens_a_workspace_and_flags_stay_flags` and `two_files_queue_a_second_tab_and_a_folder_opens_the_tree`. `document::drop_tests::two_dropped_files_open_as_tabs` and `a_dropped_directory_is_reported_and_the_buffer_stays`. `scripts/file-drop.sh` drags a file from Finder onto the window. `input::p0_tests::command_slash_comments_every_language_that_has_one`. `menu::chords::tests::menu_letters_match_the_sublime_keymap`, `menu::chords::tests::the_other_source_is_swallowed_and_a_repeat_is_not`, and `menu::tests::a_menu_comment_and_the_keymap_comment_run_once`. `render::window::tests::a_momentum_tail_keeps_each_pixel_delta`. This is not a Mac keystroke pass and not a 120 Hz measurement.

### ST-FIX-12: Panels, sidebar, tabs, and the status bar

- **Status:** Escape and a click outside the command palette, language picker, and settings panel return focus to the editor and clear the sidebar highlight. The same focus return is on the other floating panels. In the sidebar, Arrow keys move the selection, Enter opens the row, N starts a new file, F2 renames it, and Delete moves it to the Trash. Dragging a row onto a folder moves that file or folder on disk. A press that does not travel still opens the file or expands the folder. A drop onto the dragged folder itself, or onto a name that already exists, leaves the files where they were. View: Reveal Active File expands to the open file and focuses the tree. A dirty tab draws a dot. Middle-click closes the tab under the pointer. Dragging a tab reorders the strip. A wheel over the strip scrolls tabs that do not fit. The status bar shows one-based Ln/Col, the selection size, language, encoding, EOL, indent, and the git branch already known to the app. Ln/Col opens Goto Line. Language opens the language picker. EOL, indent, and encoding still toggle. The branch click opens the branch list. A checkout runs only when the worktree is clean, and creating a branch leaves the files alone. Opening a file reads `HEAD` for that name and does not walk the worktree. The language picker uses the shared selector card. Monokai gutter matches the editor background (`#272822`) and the split border is `#3E3D32`.
- **Verification:** `input::chrome_tests::escape_and_a_click_outside_return_focus_to_the_editor` goes through `handle_keyboard_event` and `handle_mouse_click_on_chrome`. `sidebar::behavior_tests::the_keyboard_creates_renames_and_trashes_and_reveal_selects_the_open_file` creates, renames, and trashes a real file. `sidebar::behavior_tests::dragging_a_file_onto_a_folder_moves_it_and_retargets_the_open_tab` moves a real file and retargets the open tab. `scripts/tree-drag.sh` drags that row in Lux.app on macOS. `document::tab_strip_tests` covers middle-click, overflow hit-testing, and drag reorder. `status_bar::tests::app_status_bar_clicks_dispatch_toggles` covers the clicks. `language_picker::tests::language_picker_click_selects_language_and_dismisses` uses the shared card geometry. `lux-ui` `monokai_chrome_uses_the_classic_wells` locks the gutter and border. This is not a Mac keystroke pass. Minimap and splits are unchanged.

### ST-FIX-11: Find, save all, tabs, zoom, and palette order

- **Status:** Command+F opens find and Option+Command+F opens replace from the physical key, so the Option glyph is not inserted. Shift+Command+F opens find in files. The find bar counter is "n of m". Option+Command+S writes every named dirty file. An untitled dirty file is left unsaved and reported. Shift+Command+T reopens the last closed tab. On macOS, Shift+Command+[ and Shift+Command+] switch tabs from the physical bracket, including when the character is a dead glyph. The terminal toggle matches the physical Backquote, so that glyph does not open the terminal. On Linux and Windows those chords stay fold and unfold, and the tab chords are Control+PageUp and Control+PageDown. Command+K Command+B toggles the sidebar. On the Sublime keymap Command+B stays Build. Command+=, Command+-, and Command+0 change the editor font and reset it to 16. Command+, opens the settings panel and Escape closes it. A repeated key-down inserts one character. A key-up inserts nothing. The system repeat interval is not measured here. The command palette draws the keymap shortcut and, with an empty query, lists the last command first.
- **Verification:** `input::chrome_tests` and `document::save_all_tests` go through `handle_keyboard_event`. `lux-search` `respects_gitignore` and `project_search::tests::project_replace_preview_and_apply` already cover project ignore rules and replace-in-files. `input::find_in_files_tests` covers F4 and Shift+F4. `input::nav_tests` covers Command+R and Command+Shift+R. `scripts/macos-keystrokes.sh` also presses Command+F, Shift+Command+F, Option+Command+F, Command+K Command+B, Command+,, Command+=, Command+-, Command+0, Shift+Command+T, and Shift+Command+[ / ]. The document line stays `alpha`. Control+Command+Up/Down is covered by `move_line_chord_swaps_the_current_line` and is not in the keystroke script, because a one-line fixture would still move if the buffer has a trailing break.

### ST-FIX-10: Palette, goto suffixes, and the remaining P0 chords

- **Status:** Command+Shift+P opens the palette, filters it, and runs Undo. Command+R lists symbols and jumps. Control+G and a typed `:line`, `@symbol`, or `#word` preview the target and Escape restores the caret. Command+click adds a cursor. Option+drag selects a column. Dragging an existing selection moves that text; Option+drag on the selection copies it, and a click that does not move places the caret. Command+Enter and Shift+Command+Enter insert a line. Command+Y redoes. Command+E fills Find from the word under the caret. One undo removes one typed word, and one undo restores a word deleted with Backspace. Command+D, Control+Command+G, Command+Shift+L, Command+L, and Command+Shift+D were already covered by `input::occurrence_tests`.
- **Verification:** `input::nav_tests` and `input::tests::command_click_adds_a_cursor_and_option_drag_selects_a_column` go through `handle_key` or `handle_keyboard_event`. `input::mouse::tests::dragging_a_selection_moves_it_and_undo_puts_it_back` moves a selection and undoes it; `option_drag_on_a_selection_copies_it` copies. `scripts/selection-drag.sh` drags the word `beta` on macOS and Option-drags a copy. `scripts/macos-keystrokes.sh` also presses Command+Shift+P, Command+R, Control+G, Command+D, Command+E, and Command+Y; the document line stays `alpha`. Autopair and word-undo tests are CI only. They are not a Mac keystroke pass.

### ST-FIX-09: Sublime chords, comments, and Enter indent

- **Status:** Command+T opens Goto Anything. On the Sublime keymap, Control+Shift+Up/Down adds a cursor on macOS. Command+] and Command+[ are keymap bindings, so a user keymap can replace them. Command+/ comments HTML, CSS, and Markdown as block comments and JSON with `//`. Command+Option+/ toggles a block comment. Command+Left uses the smart line start. Control+A and Control+E move within the line on macOS. Enter asks the parse tree for the indent level when the language has a body kind. The wheel over Goto Anything moves that list.
- **Verification:** `input::p0_tests` through `handle_key`. `scripts/macos-keystrokes.sh` also presses Command+Left, Control+A, Control+E, Control+Shift+Up, and Command+T, and the document line stays `alpha`.

### ST-FIX-08: Grammar coverage and Markdown preview

- **Status:** The languages in `crates/lux-syntax/vendor/SOURCES.md` highlight from vendored Tree-sitter grammars. Markdown paragraphs highlight emphasis, strong text, code spans, and links, and Command+Shift+V (Control+Shift+V) toggles a preview built from that parse tree. A fenced block names a language (`rust`, `python`, `ts`, and the other extensions this build highlights) and that fence is painted with the active syntax theme. The whole line is shaped once, so the spaces between tokens and the indent keep their width. A fence with no language stays one foreground color.
- **Verification:** `lux-syntax` tests `coverage`, `golden_tokens`, and `input::syntax_tests` (Makefile, Dockerfile, PHP, a Ruby shebang, Command+/ on PHP, Command+Shift+V on a Markdown buffer).

### ST-FIX-07: Tab stops and the painted caret

- **Status:** A tab advances to the next 4-column stop and is not drawn as a missing-glyph box. The caret x is the shaped cluster end, including trailing tabs and spaces. `LineLayout::width` still omits that trailing whitespace, so it is not the caret position.
- **Verification:** `text_layout::tests::a_tab_advances_to_the_next_stop_and_is_not_a_glyph` and `input::caret_tests::tab_shift_tab_and_bracket_indent_paint_the_caret_on_the_buffer` (Tab, Shift+Tab, Command+] / Command+[).

### ST-FIX-06: Panel fields, wheel, IME, and physical shortcuts
- **Status:** Implemented in the desktop event path. A point is accepted only after real keystrokes on a Mac with the Spanish layout. `scripts/macos-keystrokes.sh` is that probe on the free `macos-15` runner. If the runner cannot select Spanish, the script records `layout-unavailable` and does not report a pass.
- **Components:** `crates/lux/src/field.rs`, `crates/lux/src/overlay_wheel.rs`, `crates/lux/src/ime_hold.rs`, `crates/lux/src/app.rs`, `crates/lux/src/input/motion.rs`, `crates/lux-platform/src/desktop/convert.rs`, `crates/lux/src/search.rs`, `crates/lux-ui/src/accessibility.rs`.
- **Verification Evidence:**
  - **Releases:** key-up events return before any panel or document handler.
  - **Field chords:** Command+C/V/X/A/Z (Control elsewhere), without Option, edit the open palette, goto, language, theme, snippet, search, project-search, project, or build field. The document is unchanged.
  - **Completion:** the popup handles arrows, Enter, Tab, and Escape before document shortcuts, and a wheel moves its selection.
  - **Wheel:** an open overlay consumes the wheel. The editor `scroll_y` stays put. Trackpad pixels accumulate per overlay.
  - **IME:** a commit while a panel is open appends to that field. A commit while only the completion popup is open does not insert into the document.
  - **Search toggles:** Option+Command+R/C/W (Alt+Ctrl elsewhere) on the physical letter. The glyph `ç` is inserted.
  - **Chords:** with Command, Control, or Option held, the keymap token comes from the physical key (`f`, `/`, `[`, `]`), not the glyph the layout produced. A dead-key commit that arrives with a command chord (Spanish `` ` `` from Option+Command+[) is discarded, so Option+Command+F does not leave a grave in the document.
  - **Document text:** the editor's AccessKit value is the buffer. The macOS probe reads that value (and `AXStringForRange`) after Command+/, Option+Command+[, Option+Command+], Option+Command+F, Command+Option+S, and Command+Option+C. A pass needs a value that trims to `alpha` and no grave.
  - **Headless checks:** `input::panel_input_tests` and `desktop_pass::configured_shortcut_runs_through_the_real_keyboard_pipeline` (new tab, palette focus returns to the editor). These do not press keys through macOS.
  - **Compare panes:** `scripts/compare-captures.sh` types Command+Shift+P, Command+P, and the theme command into the Lux window. `retina-palette.png` stays an offscreen scale-2 frame.

### ST-SPLIT-01: True Multi-Pane View Architecture & Bounded Split Rendering
- **Status:** Complete & verified across macOS desktop target.
- **Components:** `crates/lux/src/pane_view.rs`, `crates/lux/src/split.rs`, `crates/lux/src/render/split.rs`, `crates/lux/src/render.rs`, `crates/lux/src/chrome.rs`.
- **Verification Evidence:**
  - **Independent Pane Architecture:** Created `PaneView` and `PaneViewport` in `crates/lux/src/pane_view.rs` (84 lines, 7 fields) managing independent viewports, active tabs, multicursors, fold states, bookmarks, and cached Tree-sitter syntax tokens per pane.
  - **Strict Geometric Bounding & Clipping:** Implemented `clamp_quad_to_rect` in `crates/lux/src/render/split.rs` to clamp all panel backgrounds, gutters, active line highlights, selection rectangles, indent guides, column rulers, and text glyphs within each pane's layout bounds, eliminating visual bleed or cross-pane overflow.
  - **Comprehensive Multi-Pane Rendering:** Each pane independently renders panel backgrounds, top accent line for active pane, gutters with line numbers and fold indicators, indentation guides, column rulers, text selections, Tree-sitter syntax-highlighted code tokens, and carets.
  - **Automated Tests:** Unit tests in `crates/lux/src/render/split.rs` (`test_clamp_quad_to_rect_inside_and_overflow`, `test_render_split_layout_bounds_and_clipping`).
  - **Tooling Passes:** `scripts/check-limits.sh`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check` all pass cleanly with 0 warnings.

### ST-SPLIT-02: Interactive Click-to-Focus, Event Dispatch & Split Pane Shortcuts
- **Status:** Complete & verified across macOS desktop target.
- **Components:** `crates/lux/src/chrome.rs`, `crates/lux/src/input/mouse.rs`, `crates/lux/src/input.rs`, `crates/lux/src/document.rs`, `crates/lux/src/split.rs`, `crates/lux/src/split/tests.rs`, `crates/lux/src/render/split.rs`, `crates/lux-config/keymaps/sublime.jsonc`, `crates/lux/src/palette/commands.rs`, `crates/lux/src/palette/registry.rs`, `crates/lux/src/input/layout_tests.rs`.
- **Verification Evidence:**
  - **Interactive Click-to-Focus & Text Hit-Testing:**
    - `handle_mouse_click_on_chrome` detects clicks inside any split pane geometry via `crate::split::hit_test_pane` and transfers active focus via `app.focus_group(pane_idx)`.
    - Returns `false` upon interior pane click (leaving `true` solely for divider dragging), allowing `crates/lux/src/input/mouse.rs` (`position_at_mouse` and `begin_mouse_selection`) to place the cursor directly at the clicked text coordinates.
    - Enables typing, backspacing, selecting, copying, and pasting in the newly focused pane in a single click.
  - **Visual Focus Indicators & Static Carets:**
    - Active pane renders an accent-colored top border line (`theme.accent`) clipped to pane bounds.
    - Inactive panes omit the top accent line.
    - Active pane renders standard blinking carets (`cursor_glyph.visible`), while inactive panes render subtle static carets (`alpha: 0.65`, non-blinking) preserving visual caret position without distraction.
  - **Keyboard Navigation & File Migration Shortcuts:**
    - Connected `focus_group_1..4` (`Ctrl+1..4`) to focus groups 0..3 directly from the keyboard.
    - Connected `move_to_group_1..4` (`Ctrl+Shift+1..4`) to transfer the active file to group 0..3, cleanly reassigning available open tabs to the vacated pane.
    - Protected `Ctrl+1..4` in `crates/lux/src/document.rs` when split layout is active so it is not intercepted as tab switching on Linux/Windows.
    - Implemented progressive layout step-down in `close_active_split` (`Columns4 -> Columns3 -> Columns2 -> Single`, `Rows3 -> Rows2 -> Single`, `Grid2x2 -> Columns2`).
  - **Comprehensive Automated Tests:**
    - Verified in `crates/lux/src/input/layout_tests.rs`:
      - `test_pane_click_focus_switching`: verifies clicking between panes toggles active pane focus without swallowing clicks.
      - `test_split_click_focus_and_interactive_editing`: verifies clicking inactive pane, typing, deleting, switching panes, and typing with full state and edit retention.
      - `test_ctrl_numbers_focus_and_move_shortcuts`: verifies `Ctrl+1..4` and `Ctrl+Shift+1..4` key events via dispatch.
      - `test_split_visual_focus_indicator_and_static_cursors`: verifies quad instances for top accent border and static carets.
    - Unit tests in `crates/lux/src/split/tests.rs` verifying layout presets, group switching, buffer edit synchronization, and geometry calculation.
  - **Tooling Passes:**
    - 252/252 tests pass in `cargo test -p lux`.
    - Full workspace test suite passes cleanly (`cargo test --workspace`).
    - `scripts/check-limits.sh` passes (< 600 lines/file, <= 25 fields/struct).
    - `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` pass with 0 warnings.

### E13 / UI-SETTINGS: Settings page

- **Status:** Command+, and Preferences: Open Settings UI open an opaque settings page between the tab bar and the status bar. The editor text is under that page. The page edge is the neutral chrome border, not the theme accent. The sidebar is All, Editor, Appearance, Keys, Files, Git, and CLI. Search filters every category. A selected row is inset inside the page. Labels are at least 13pt. A boolean is a toggle, a number is a stepper, and a theme or keymap is a menu. Git and CLI rows describe the real palette commands and do not write a setting. The wheel, Page Down, Page Up, End, and Home scroll the page and leave the editor scroll where it was. Escape closes an open menu first, then the page. The command palette draws match highlights, shows each command's shortcut, and closes on Escape. Its selection is the panel-row fill. Confirming a theme keeps the buffer, cursor, and scroll, and the next launch reads that theme back.
- **Verification:** `settings::visual_tests::settings_panel_has_no_debug_magenta_border`, `settings::visual_tests::settings_selection_stays_inside_the_card`, and `settings::visual_tests::settings_row_lays_out_label_description_and_value`. `settings::visual_tests::settings_toggle_stepper_and_menu_change_the_stored_value` changes ligatures, font size, and the color theme. `settings::visual_tests::settings_wheel_scrolls_the_page_and_not_the_editor`, `settings::visual_tests::end_and_home_scroll_the_settings_page`, and `settings::visual_tests::escape_closes_the_menu_before_the_page`. `settings::tests::theme_reload_keeps_the_buffer_and_scales_line_height` cycles every theme setting and reloads Solarized Light. `render::panel::contrast_tests::ten_bundled_themes_paint_ui_text_at_wcag_aa` and `ten_bundled_themes_paint_controls_at_wcag_aa` compute WCAG AA for text, the toggle, the selected category, and the stepper value and chips on that selection. `scripts/settings-e2e.sh` is the macos-15 job. `docs/compare/settings.png` is `lux --settings` rendered at scale factor 2.

