# UI review evidence (origin/main 8d3effb)

Release build, macOS, renderer snapshot at scale 2 (`--snapshot-on-usr1`,
SIGUSR1), driven with real CGEvents through `scripts/macos-drive.sh`.
Window 1000×700 pt. Temporary `HOME`; fixture is a small git repo with one
modified file (`src/main.rs`: 3 added lines, 1 modified, 1 deleted) and one
untracked file (`NEW.md`). PNGs recompressed losslessly with ImageMagick.

| File | Surface |
|---|---|
| 01-editor-sidebar-tabs | Sidebar on folder, 3 tabs, dirty active tab, status bar, minimap |
| 02-split | Two columns (Cmd+Alt+2) |
| 03-palette-query | Command palette, query `sel` |
| 04-goto | Goto Anything, query `stat` |
| 05-find / 06-replace | Find bar (Cmd+F) / replace (Cmd+Alt+F), query `counts` — snapshot layout differs from the live window, see 24 |
| 07-find-in-files | Find Results for `counts` |
| 08-language-picker | Change Language Mode |
| 09-theme-picker | `--theme` |
| 10-settings | `--settings` |
| 11-completion | `cou` typed — no completion popup (no buffer-word source; rust-analyzer not installed) |
| 12-build | Cmd+B on a Cargo project, bottom panel |
| 13/14/15/21/22 | GitHub Light: editor, palette, find results, settings, goto |
| 16-empty | Window with no arguments |
| 17-rust-git | Rust file with git gutter marks |
| 18-many-tabs | Nine tabs, overflow |
| 19-long-file | Scrolled long Rust file, minimap viewport, indent guides |
| 20-selection | Selection on the current line (Monokai) |
| 23-solarized-light-editor | Solarized Light |
| 24-live-replace-window | `screencapture -l` of the live window, replace bar open |
| 25-live-build-panel-window | `screencapture -l` of the live window, build panel open |
