# Roadmap

Checked against `main` `197353c` on 2026-11-01, then updated in the
phase 0 change. This page is the parity plan
for the macOS product. [NON-GOALS.md](NON-GOALS.md) still excludes a
debugger, live collaboration, a hosted marketplace, accounts, telemetry,
and AI chat. Packaging and CI stay as they are unless something is broken.
Windows is out of this plan.

A phase closes only when its acceptance evidence is in the tree: real
window captures from the compare job below, and, from phase 3 on, numbers
from the benchmark method below. A version tag that only changes packaging
or docs is not a delivery.

Legend: **hecho** means a check on this commit showed the behavior.
**parcial** means the code or a unit test exists and the macOS window does
not yet meet the acceptance bar. **falta** means it is not there.

## What this commit actually does

On Linux, `cargo test --workspace` passed (988 tests, 31 ignored) and
`cargo build -p lux` produced a debug binary. Under Xvfb with lavapipe the
process loads DejaVu Sans Mono and stays up. This machine has no Metal
adapter, so those pixels are not the product.

The product look is the macOS capture from Actions run
[41296991287](https://github.com/lcajigasm/lux/actions/runs/41296991287)
(`docs/screenshots/editor.png`, `palette.png`, `split.png`): a 1000×700
window of `docs/screenshots/example.rs` on standard `macos-14`. The editor
surface is the light theme. The command palette is a flat dark rectangle
whose rows cannot be read. The status bar is a dark strip whose labels
cannot be read. Syntax colors are pale. There is no Monokai window in that
capture.

## Why the window looks wrong

Four separate mechanisms, all visible in that capture or in the draw path
that produced it:

The capture above is the previous build. `docs/compare/monokai-side-by-side.png`,
`docs/compare/characters.png`, and `docs/compare/selectors.png` are the
macos-15 windows from compare run 41363899505. 0.1 and 0.2 hold. The Lux
palette in that file is the wider card with the query `sel`, under the
tab bar. Go to file with `mon` lists `monokai.rs`, as Sublime and Zed do.
`retina-palette.png` is that frame at scale factor 2. The runner
is 1×, so Sublime is not in the scale-2 file. Block 0 is closed.

1. **Monokai is the launch theme.** `workbench.colorTheme` defaults to
   `monokai`. `Chrome` starts on `Theme::MONOKAI` and does not follow the
   OS until the setting is `system`. The colors are parsed from
   `themes/Monokai.sublime-color-scheme` (MIT, see `themes/NOTICE.md`).
   More `*.sublime-color-scheme` files load from the user config
   `themes/` directory.

2. **Chrome colors come from the theme.** Sidebar, tabs, status bar,
   palette, find, goto, and the other selector panels use `theme.chrome`
   instead of a fixed indigo quad. Text on those surfaces uses
   `theme.foreground` against that fill. Git added/removed colors and
   diagnostic red/amber stay semantic.

3. **Glyphs are grayscale masks at every scale.** Rasterization uses
   `TextRenderingMode::Grayscale`, including scale 1. Quads are drawn
   first, so the mask blends with the surface already in the framebuffer.
   Hinting is unchanged. A cluster paints every glyph parley produced,
   including a combining mark.

4. **Selectors are rounded and move.** The command palette, goto, theme
   picker, and completion list draw a shadowed rounded surface (radius 0
   on every other quad is still a sharp rectangle), a geometric icon per
   row, and a short ease. The ease uses `request_repaint_at` plus the
   platform wakeup the caret blink already uses. Reduced motion settles
   on the first frame. Fuzzy match highlighting stays.

No font is bundled. The macOS loader tries `SFNSMono.ttf`, then the Xcode
SF Mono files, then `Menlo.ttc` face 0. Editor ASCII in the capture is a
real monospace face. Ligatures default on for the buffer and off for every
chrome string.

## Block 0 — Visual quality

This block is first. Order inside it is fixed.

### 0.1 Encoding — hecho

| Check | State |
|---|---|
| UTF-8, including `é`, `ñ`, emoji, and CJK, decodes and round-trips | hecho. `docs/compare/fixtures/characters.txt` plus the encoding tests |
| UTF-8 BOM and UTF-16 LE/BE **with** BOM | hecho. On-disk fixtures, byte-identical save |
| UTF-16 without a BOM | hecho when one byte of each pair is mostly NUL and the decode scores as text. Shorter, odd-length, or pure-CJK input without a BOM stays binary |
| Windows-1252 | hecho when the file is not UTF-8 and contains a byte in `0x80..=0x9F`. Other invalid UTF-8 stays Latin-1. Undefined 1252 bytes keep the C1 control so the byte round-trips |
| Grapheme movement | hecho in `lux-core`. Shaping emits every glyph in a cluster, so a combining mark is not dropped when the font keeps it as a second glyph |
| Encoding label in the status bar | hecho. Labels include `UTF-8`, `UTF-8 BOM`, `UTF-16 LE`, `UTF-16 BE`, `Windows-1252`, and `Latin-1` |
| Save round-trip | hecho. `Buffer::save` writes `encoding::encode` and refuses the write when `can_encode` is false. The fixture test asserts the bytes |
| Sample files in the repo | hecho, under `docs/compare/fixtures/` |

Acceptance: fixture files under `docs/compare/fixtures/` cover `ñ`, áéíóúü,
emoji, CJK, and a combining sequence; plus Latin-1, Windows-1252, UTF-8
with and without BOM, and UTF-16 LE/BE with BOM. Opening each file shows
the same text the fixture names, the status bar shows the encoding, and
save without an explicit conversion is byte-identical. A requested
conversion is the only path that may change bytes. Tests open those files;
they do not rebuild the bytes in the assertion.

### 0.2 Monokai as the default dark theme — hecho

The default is Monokai. `themes/Monokai.sublime-color-scheme` is parsed at
startup and a test locks it to `Theme::MONOKAI` and the Monokai syntax
scopes. Sidebar, tabs, palette, goto, and the status bar use that chrome.
`docs/compare/monokai-side-by-side.png` is the macos-15 capture that closes
this row: Lux `#272822` / `#1E1F1C` / `#414339` beside Sublime on the same
file. Sublime's minimap and lighter window chrome are Sublime's own UI.

Acceptance: `workbench.colorTheme` defaults to Monokai. One vendored
`.tmTheme` or `.sublime-color-scheme` is the source of those colors, with
the upstream URL, author, and license text in the repo. The loader reads
further theme files from the user config directory. Editor, sidebar, tabs,
palette, goto, autocomplete, and status bar all use that palette. Monokai
Pro is proprietary and is not a candidate. A file with no license is not a
candidate either; the VS Code `theme-monokai` sources are MIT and are the
first file to check. Sublime's bundled scheme is used only if its license
allows redistribution.

### 0.3 Text and selectors — hecho

Text at every scale, including scale 1, rasterizes a grayscale mask and
blends it with the quad already drawn for that surface. Selector glyphs
are grayscale too, so they composite on the panel instead of the
editor-background uniform. Ligatures
and the SF Mono fallback list were already there. The command palette,
go-to-file, and theme picker are about 46% of the window, sit below the
tab bar with a gap, and draw a lifted card (`#414339`), a light rim, a
dark input well, and a drop shadow. Placeholders are "Execute a command…",
"Go to file…", and "Select a color scheme…". Rows are 36px, labels use the
foreground color, the selected row is `#75715E` with an accent bar, and
fuzzy hits use the accent. The right-hand label is the real keymap chord
when one exists. Compare run 41348580486 is the capture Forge rejected.
Compare run 41363899505 shows the card under the tab bar, the query, the
editor beside it, and go to file listing only `monokai.rs` for `mon`,
beside Sublime and Zed. A match on the beige selection is the
foreground, drawn twice; other rows keep the accent. Ranking keeps a
contiguous or word-initial match and drops scattered names when one
exists. The runner logged display scale 1. `retina-palette.png` is
the scale-2 framebuffer (2000×1312). Forge accepts that file as Lux's
Retina render; Sublime cannot be captured at that scale on the 1× runner.
Open motion is a 120 ms fade and an 8 px slide. The still is the settled
frame. The three samples are
`overlay_motion::the_open_animation_fades_and_slides_across_three_frames`.

Acceptance, on top of 0.1 and 0.2: Retina text is grayscale-antialiased (or
otherwise correct for a 2× framebuffer), with ligatures, a stable line
height, and SF Mono when the file exists, otherwise Menlo, otherwise a
bundled monospace face. The command palette, goto-file, theme picker, and
completion list do fuzzy match, highlight the matched characters, show
icons, and use a rounded shadowed surface. Motion is short and does not
block input. Compare run 41363899505 holds. 0.3 holds with block 0.

### Block 0 is closed only when these captures exist

Both files are produced by the compare job on standard `macos-15` and
committed under `docs/compare/`. `macos-14` is unsupported after
2026-11-02; the captures already in `docs/screenshots/` were taken on it
before that.

- `docs/compare/monokai-side-by-side.png` — Lux with Monokai beside Sublime
  Text with Monokai, same file, same window size, same display resolution.
- `docs/compare/characters.png` — the Lux window on the special-character
  fixture, same resolution.
- `docs/compare/selectors.png` — Lux, Sublime Text, and Zed with the
  command palette open and the query `sel`. Compare run 41363899505.
  `goto.png` in that run is go to file with `mon`: Lux, Sublime, and Zed
  each list `monokai.rs`. The runner is 1×. `retina-palette.png` is Lux's
  renderer at scale 2. Forge accepts the palette, the goto list, and that
  framebuffer. Block 0 is closed.

If a later capture loses the comparison, the block opens again and no
release is tagged for that regression.

### How the compare job works

`.github/workflows/compare.yml` runs on `pull_request` and
`workflow_dispatch` (this token cannot start `workflow_dispatch`). One
standard `macos-15` job, timeout 45 minutes. `scripts/compare-captures.sh`
builds debug `lux`, installs the Sublime Text cask, opens
`docs/compare/fixtures/monokai.rs` in both apps with Monokai, and
`docs/compare/fixtures/characters.txt` in Lux. `screencapture -l` captures
the real windows. ImageMagick appends the pair. The same job then
installs the Zed cask, launches `target/Lux.app` with no `--palette`
and no `--query`, posts Command-Shift-P and types `sel` into that app,
asks Sublime for its command palette with the same text, sends Zed
Cmd-Shift-P and types `sel`, and appends the three windows to
`selectors.png`. `retina-palette.png` is that same live Lux.app frame,
written when the job sends SIGUSR1 after the typed query, at scale
factor 2. It then opens go to file and the theme picker with key posts
(`mon` and `mo`), then the fixtures folder for `tree.png` and
`monokai.rs` plus `characters.txt` for `tabs.png`.
On a 1× runner, Sublime cannot be screencaptured at scale 2, and the
script does not upscale a 1× shot. If Sublime or Zed cannot be captured,
the script exits
non-zero and does not write a stand-in image. The job uploads the PNGs.
Committing them is a separate change. The artifact alone does not close
the phase.

## P0 evidence — A10, I2, I4

These three rows are the DoD for real interaction on the free `macos-15`
runner. The captures are the compiled app. A panel opened by
`--palette`, `--query`, or `--settings` does not count.

| Id | Proof | Run |
|---|---|---|
| A10 | `compare.yml` stages `target/Lux.app` and launches it with no `--palette` and no `--query`. It posts Command-Shift-P, types `sel`, and uploads `selectors.png` (Lux beside Sublime Text and Zed). `retina-palette.png` is that same session after SIGUSR1, at renderer scale 2 (2000×1312 on that run). | [42588286845](https://github.com/lcajigasm/lux/actions/runs/42588286845) |
| I2 | `evidence.yml` launches that app, types `LuxEvidence42`, then posts Command-P, Command-Shift-P, Command-D, Command-Comma, a scroll, and Command-S. The job fails unless the opened file contains the token and the process is still alive. That run saved the token and left the process running. | [42588287110](https://github.com/lcajigasm/lux/actions/runs/42588287110) |
| I4 | The same evidence job writes `editor.png`, `palette.png`, and `settings.png` from the live session at scale 2, after the typing, the `sel` query, and Settings. Sublime Text and Zed install free in the compare job, but the runner display is 1×, so those scale-2 frames are Lux only. Their 1× windows stay in `selectors.png`. Each frame on that run is 2000px wide. | [42588287110](https://github.com/lcajigasm/lux/actions/runs/42588287110) |

## P0 macOS CI — A9, C2, C4, E1, I3, H1–H8

`p0-macos.yml` runs on the free `macos-15` runner. The shortcut, editing,
and checklist jobs post CGEvents to `target/Lux.app`. The performance job
uses the release binary. A missed H budget is written in the artifact and
does not fail the job. Run URLs land here after the jobs finish.

| Id | What the job checks | Run |
|---|---|---|
| A9 | Block 3 chords on US, and on Spanish when the runner can select that layout. Each chord changes the file or the probe. A missing Spanish layout is `layout-unavailable`, not a pass. | pending |
| C2 | Typed `()[]{}""` and `''`, including the skip of a closer and backspace of a pair. The saved file is the assertion. | pending |
| C4 | `ab`, a pause, `c`, then Command-Z restores `ab` and the caret. `hello world` then Command-Z restores `hello ` and the caret. | pending |
| E1 | solo-Mac. `displayplacer list` on macos-15 showed one 1024×768 screen, `scaling:off`, and modes from 640×480 to 1920×1080. None had `scaling:on`, so a screencapture of the window cannot be scale 2. | [42593509340](https://github.com/lcajigasm/lux/actions/runs/42593509340) |
| I3 | `docs/receipts/p0-macos-ci.md` is written by the checklist job from that run. | pending |
| H1 | Cold start to the first presented frame, median of 10, release binary. `h1 cold start (macos-15)` fails when a sample or `cold.png` is missing. | pending |
| H2 | Key event to the next presented frame, p99, 20k-line file. | pending |
| H3 | Warm process, SIGUSR2 opens a 10 MB file, time to that buffer's first presented frame. | pending |
| H4 | Same open, 100 MB. | pending |
| H5 | Slowest redraw while scrolling a 50k-line file. | pending |
| H6 | Extra RSS across the 100 MB open, against 3× the file. | pending |
| H7 | Sublime and Zed are timed only until a window is listed. That is not H1's clock. H2 and H5 have no equivalent outside the process. | pending |
| H8 | The table in `docs/PERFORMANCE.md` copied from the performance artifact. | pending |

## Block 1 — Interface — parcial

| Surface | State on 44684ab |
|---|---|
| Theme | hecho for the editor and chrome wells. `docs/compare/monokai-side-by-side.png` |
| Typography | parcial. System monospace loader works (DejaVu here, Menlo/SF Mono path on macOS). No bundled face. HiDPI uses grayscale masks; the committed capture is 1×. |
| File tree | hecho. Compare run 41394350609, `docs/compare/tree.png`. Lux, Sublime Text, and Zed are open on `docs/compare/fixtures`. Lux lists `notes` (disclosure and folder mark) and the files under the header `FIXTURES`. Names are the theme foreground on the Monokai sidebar. Sublime's sidebar in that shot is its light chrome. The runner display scale is 1 |
| Tabs | hecho. The same run, `docs/compare/tabs.png`. `lux monokai.rs characters.txt` shows both tabs, as Sublime and Zed do. The active tab is `characters.txt`, the editor well, with the accent bar. The inactive title is foreground on `tab_inactive` |
| Status bar | parcial. Items are built and painted; the capture cannot be read |
| Minimap | parcial. A muted strip is in the capture. It is not a readable map of the file |
| Splits | parcial. `split.png` shows two columns of the same file. The gutter and divider are the dark hardcoded chrome |
| Command palette | parcial. Compare run 41348580486 shows Lux, Sublime, and Zed with the palette open. Lux rows have icons and a rounded surface. The still does not show the ease |
| Settings | parcial. `workbench.colorTheme` and `workbench.keymap` load from JSON. There is no settings editor that meets the visual bar |

Acceptance: every row is visible in a `docs/compare/` capture of one
folder, two splits, the palette, and the settings surface, themed with
Monokai, with the status bar readable.

## Block 2 — Functionality — parcial

| Capability | State |
|---|---|
| Multicursor | parcial. `select_next_occurrence` and column select are implemented and unit-tested. No Mac capture |
| Find and replace in the file | parcial. Headless session drives find. The find bar is one of the hardcoded dark panels |
| Find in the project | parcial. The command and a renderer exist. No capture of a real result list |
| Go to file and go to symbol | parcial. Goto Anything and `Cmd+Shift+R` are bound. The panel is a rectangle. Block 0.3 |
| Syntax highlighting | parcial. Tree-sitter colors the capture, in the light palette |
| Undo and redo | hecho at the buffer. `Cmd+Z` / `Cmd+Shift+Z` are bound. No Mac capture |
| Sessions | parcial. Recovery snapshot and `save_session` exist. The capture does not restore a session |
| Basic LSP | parcial. A client, completion renderer, and diagnostics path exist in the shell. No language server was started on this machine |

Acceptance: one capture per row on the fixture project, plus a test that
opens a file, types, undoes, redoes, finds, jumps to a symbol, and asks a
real `rust-analyzer` (or the repo's chosen server) for one completion. The
server binary is installed on the `macos-15` job, not stubbed.

## Block 3 — Shortcuts — parcial

The default keymap is Sublime (`workbench.keymap`). Bindings live in
`crates/lux-config/keymaps/sublime.jsonc` with a `mac` chord. The keymap
file is the configuration surface. None of these chords was pressed on a
Mac keyboard in this pass. Headless dispatch covers palette, split, and
find.

| Chord | Command | State |
|---|---|---|
| `Cmd+P` | `quick_open` | parcial |
| `Cmd+Shift+P` | `command_palette` | parcial. `input::nav_tests` opens it and runs Undo |
| `Cmd+D` | `select_next_occurrence` | parcial |
| `Cmd+K Cmd+D` | `skip_next_occurrence` | parcial |
| `Cmd+Shift+L` | `split_selection_into_lines` | parcial |
| `Cmd+/` | `toggle_comment` | parcial |
| `Ctrl+Shift+Down` | `add_cursor_below` | parcial. Sublime macOS chord. `input::p0_tests` |
| `Ctrl+Shift+Up` | `add_cursor_above` | parcial. Sublime macOS chord. `input::p0_tests` |
| `Cmd+F` | `find` | parcial. Headless session drives it |
| `Cmd+Alt+F` | `replace` | parcial |
| `Cmd+Shift+F` | `find_in_files` | parcial |
| `Cmd+G` / `Cmd+Shift+G` | `find_next` / `find_prev` | parcial |
| `Cmd+R` / `Cmd+Shift+R` | `goto_symbol` / `goto_symbol_in_project` | parcial. `input::nav_tests` jumps to `@symbol` |
| `Cmd+S` | `save` | parcial. Headless session saves |
| `Cmd+Z` / `Cmd+Shift+Z` | `undo` / `redo` | parcial |
| `Cmd+Alt+2` | `set_layout_columns_2` | parcial. Headless session splits |
| `Cmd+L` | `expand_to_line` | parcial |
| `Cmd+Shift+D` | `duplicate_line` | parcial |
| `Cmd+Shift+K` | `delete_line` | parcial |

Acceptance: the table above, each chord marked hecho only after the
compare job's Mac session sends that chord to the Lux window and the
capture (or a log the test asserts) shows the effect. A user keymap file
overrides one of these chords without editing the binary.

## Block 4 — Performance — falta

`docs/PERFORMANCE.md` records component timings from a MacBook Pro in July
and August 2026 (buffer insert, a 100 MB decode, tree-sitter on a range).
Those numbers are not a cold start of `lux`, not an open of a 10 MB file
in the window, and not keypress-to-pixel. This plan does not repeat them
as current results. No Sublime or Zed number is listed here because none
was measured.

Method, release binary, same machine for every column:

| Measurement | Method | Target |
|---|---|---|
| Cold start | Process start until the first presented frame, empty window, median of 10 | < 150 ms |
| Open 10 MB | UTF-8 fixture, process already warm, open until first frame of that buffer, median of 10 | < 150 ms |
| Open 100 MB | Same, plus peak RSS minus idle RSS | < 1 s and extra RSS < 3× the file |
| Typing latency | Hold a key, p99 of keypress to the frame that contains the new glyph, 20k-line file | < 8 ms |

The CI typing test `test_continuous_typing_latency_large_file_under_8ms`
times `insert_at_cursor` plus syntax sync. It stays. It is not the row
above.

Sublime and Zed join the table only on a run that times them the same way
on that machine (`brew --cask`, no license). Until that run is committed,
the comparison cells stay empty.

## Phases

Each phase ends with new `docs/compare/` captures from the job above. Phase
3 also commits the benchmark table. No release is tagged for a phase that
missed its bar.

1. **Phase 0, closed.** 0.1 and 0.2 hold. Compare run 41363899505
   shows the wider palette with the editor beside it, go to file with
   `mon` listing `monokai.rs`, and `retina-palette.png` as the scale-2
   framebuffer. The runner is 1×. Block 0 is closed. The release is v1.4.0.
2. **Phase 0b.** The selector capture against Sublime and Zed. Compare
   run 41363899505 holds. That close is the first release this plan allows.
3. **Phase 1.** Block 1 stays parcial. File tree and tabs are hecho on
   compare run 41394350609 (`docs/compare/tree.png`, `docs/compare/tabs.png`).
   Status bar, minimap, splits, and settings are still open. The release
   for the two closed rows is v1.5.0.
4. **Phase 2.** Blocks 2 and 3 together. A command counts when its Sublime
   chord works on the Mac and the capture shows it.
5. **Phase 3.** Block 4, on the binary phases 0–2 produced.

## Open pull requests

Reviewed 2026-11-01. All six are drafts from 2026-09-01. #9–#13 stack on
#8, which targets an old `main`. The behavior they add is already on
`44684ab` in later form: system fonts (`crates/lux/src/font.rs`), open and
save (`crates/lux-core/src/buffer/save.rs`), settings and the Sublime keymap,
syntax in the paint path (the capture is colored), tabs, and a status bar.
Rebasing the stack would replay obsolete copies of `app.rs` and `render.rs`.
Branches stay.

| PR | Decision | Reason |
|---|---|---|
| [#8](https://github.com/lcajigasm/lux/pull/8) | Close | September integration spine (`TODO.md`, ADR 0017). This roadmap replaces it |
| [#9](https://github.com/lcajigasm/lux/pull/9) | Close | System monospace loading already runs (DejaVu on Linux, SF Mono then Menlo on macOS). Font quality is phase 0.3, not this branch |
| [#10](https://github.com/lcajigasm/lux/pull/10) | Close | Open and save already round-trip through `Buffer::save`. Encoding gaps are phase 0.1 |
| [#11](https://github.com/lcajigasm/lux/pull/11) | Close | Settings and `sublime.jsonc` are already the running keymap. Shortcut proof is phase 2 |
| [#12](https://github.com/lcajigasm/lux/pull/12) | Close | Highlights already reach the window. Faithful Monokai scopes are phase 0.2 |
| [#13](https://github.com/lcajigasm/lux/pull/13) | Close | Tabs and the status bar already paint. Making them readable is phase 0.2 and phase 1 |

No other pull request is open.
