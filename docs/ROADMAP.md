# Roadmap

Checked against `main` `44684ab` on 2026-11-01. This page is the parity plan
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

1. **The default theme is light, and Monokai is not what launches.**
   `workbench.colorTheme` defaults to `system`
   (`crates/lux/src/settings.rs`). On the light `macos-14` runner that
   selects Lux Light (`#FAFAFD` chrome, pale syntax tokens). Monokai exists
   only as handwritten constants (`#272822`, `#F8F8F2`, `#F92672`,
   `#E6DB74` in `crates/lux-ui/src/theme.rs` and
   `crates/lux-syntax/src/theme/presets.rs`). Nothing loads a `.tmTheme`.

2. **Chrome colors ignore the theme.** The tab strip, inactive tabs, status
   bar, command palette, and find panel are hardcoded indigo quads
   (`[0.055, 0.055, 0.075]`, `[0.09, 0.09, 0.12]`, and the same family in
   `crates/lux/src/render.rs`, `status_bar.rs`, `palette.rs`). Text on those
   quads uses `theme.foreground`. On Lux Light that foreground is `#1E1E24`,
   so the palette and the status bar draw near-black glyphs on a near-black
   quad. The active tab uses `theme.background`, which is why the one tab
   in the capture is readable and the palette is not. The status-bar string
   itself is real (`UTF-8`, `LF`, the cursor) and is painted; the capture
   cannot be read because of this contrast.

3. **Subpixel glyphs are composited against one background.** Every glyph
   is rasterized with `TextRenderingMode::Subpixel` and hinted
   (`GlyphAtlas::rasterize` calls `hint(true)`). `text.wgsl` mixes that LCD
   mask with a single uniform, `theme.background`, and writes an opaque
   pixel. On a Retina framebuffer the LCD assumption is wrong, and any
   glyph that is not sitting on the editor fill (palette, status bar,
   selection, gutter) is blended against the wrong color. Layout does scale
   with the framebuffer: `FONT_SIZE` is 16 and positions are multiplied by
   `scale_factor`, and the surface size is the physical size. The blur is
   the AA mode and the single clear color, not a missing scale multiply.

4. **Selectors are rectangles.** The palette, goto, and theme picker draw
   axis-aligned quads. The palette renderer has no match highlight, no
   icons, no corner radius, and no motion. "Shadow" in the find and LSP
   panels is another flat quad.

No font is bundled. The macOS loader tries `SFNSMono.ttf`, then the Xcode
SF Mono files, then `Menlo.ttc` face 0. Editor ASCII in the capture is a
real monospace face. Ligatures default on for the buffer and off for every
chrome string.

## Block 0 — Visual quality

This block is first. Order inside it is fixed.

### 0.1 Encoding — parcial

| Check | State |
|---|---|
| UTF-8, including `é` and emoji, decodes and round-trips | hecho, in memory (`lux-core` encoding tests, `hola señor 😀` save test) |
| UTF-8 BOM and UTF-16 LE/BE **with** BOM | hecho, in memory |
| UTF-16 without a BOM | falta. NUL bytes take the binary path and the file is refused |
| Windows-1252 | falta. The name does not appear in the tree. Invalid UTF-8 falls through to Latin-1, so `0x80`–`0x9F` become C1 controls instead of the 1252 characters |
| Grapheme movement | hecho in `lux-core` (`next_grapheme`). The live capture file is ASCII, so the window has not shown combining marks, emoji, or CJK |
| Encoding label in the status bar | parcial. The label is `UTF-8` / `UTF-16 LE` / `Latin-1` and the capture cannot show it (see the contrast bug above) |
| Save round-trip | parcial. `Buffer::save` writes `encoding::encode` and refuses a Latin-1 save that would drop a character above U+00FF. There is no on-disk fixture. `tests/fixtures/basic.txt` is the ASCII line `hello world` |
| Sample files in the repo | falta |

Acceptance: fixture files under `docs/compare/fixtures/` cover `ñ`, áéíóúü,
emoji, CJK, and a combining sequence; plus Latin-1, Windows-1252, UTF-8
with and without BOM, and UTF-16 LE/BE with BOM. Opening each file shows
the same text the fixture names, the status bar shows the encoding, and
save without an explicit conversion is byte-identical. A requested
conversion is the only path that may change bytes. Tests open those files;
they do not rebuild the bytes in the assertion.

### 0.2 Monokai as the default dark theme — parcial

Classic hex values exist as Rust constants and can be selected by name.
They are not the default, they do not paint the sidebar, tabs, palette, or
status bar, and they were not imported from a theme file.

Acceptance: `workbench.colorTheme` defaults to Monokai. One vendored
`.tmTheme` or `.sublime-color-scheme` is the source of those colors, with
the upstream URL, author, and license text in the repo. The loader reads
further theme files from the user config directory. Editor, sidebar, tabs,
palette, goto, autocomplete, and status bar all use that palette. Monokai
Pro is proprietary and is not a candidate. A file with no license is not a
candidate either; the VS Code `theme-monokai` sources are MIT and are the
first file to check. Sublime's bundled scheme is used only if its license
allows redistribution.

### 0.3 Text and selectors — falta

The draw path exists and is the path that produced the bad capture.

Acceptance, on top of 0.1 and 0.2: Retina text is grayscale-antialiased (or
otherwise correct for a 2× framebuffer), with ligatures, a stable line
height, and SF Mono when the file exists, otherwise Menlo, otherwise a
bundled monospace face. The command palette, goto-file, theme picker, and
completion list do fuzzy match, highlight the matched characters, show
icons, and use a rounded shadowed surface. Motion is short and does not
block input.

### Block 0 is closed only when these captures exist

Both files are produced by the compare job on standard `macos-14` and
committed under `docs/compare/`:

- `docs/compare/monokai-side-by-side.png` — Lux with Monokai beside Sublime
  Text with Monokai, same file, same window size, same display resolution.
- `docs/compare/characters.png` — the Lux window on the special-character
  fixture, same resolution.

If Lux loses that comparison, the block stays open and no release is tagged.

### How the compare job works

Not added in the plan change. When phase 0 starts, add
`.github/workflows/compare.yml`:

- `workflow_dispatch` only, `runs-on: macos-14`, timeout 45 minutes. It
  does not run on pull requests.
- Build `lux` in release mode.
- `brew install --cask sublime-text` and `brew install --cask zed`. No
  license, no account. If a cask name has moved, the job log is the
  record and the workflow is updated; the job does not fall back to a
  drawing.
- Fixtures live in `docs/compare/fixtures/`. Sublime opens the same file
  with its built-in Monokai color scheme. Zed opens it for the selector
  and text frames in 0.3.
- AppleScript sets each window to the same bounds. `screencapture -l`
  captures the window id whose owner is `lux`, `Sublime Text`, or `Zed`.
- ImageMagick appends the pair into the two files above. A third frame,
  `docs/compare/selectors.png`, is the palette and goto of all three apps
  on the same query; 0.3 does not close without it.
- The job uploads the PNGs as an artifact. A person commits them. The
  artifact alone does not close the phase.

## Block 1 — Interface — parcial

| Surface | State on 44684ab |
|---|---|
| Theme | parcial. Named themes exist; the running default is `system` → Lux Light. Block 0.2 owns the fix |
| Typography | parcial. System monospace loader works (DejaVu here, Menlo/SF Mono path on macOS). No bundled face, chrome AA is wrong. Block 0.3 |
| File tree | parcial. Sidebar code runs when a folder is open. The capture opened one file, so the tree was absent |
| Tabs | parcial. One readable tab in the capture; inactive tabs use the hardcoded dark quad |
| Status bar | parcial. Items are built and painted; the capture cannot be read |
| Minimap | parcial. A muted strip is in the capture. It is not a readable map of the file |
| Splits | parcial. `split.png` shows two columns of the same file. The gutter and divider are the dark hardcoded chrome |
| Command palette | parcial. `Cmd+Shift+P` is bound and the headless session opens it. The capture of the open palette is illegible. Block 0.3 |
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
server binary is installed on the `macos-14` job, not stubbed.

## Block 3 — Shortcuts — parcial

The default keymap is Sublime (`workbench.keymap`). Bindings live in
`crates/lux-config/keymaps/sublime.jsonc` with a `mac` chord. The keymap
file is the configuration surface. None of these chords was pressed on a
Mac keyboard in this pass. Headless dispatch covers palette, split, and
find.

| Chord | Command | State |
|---|---|---|
| `Cmd+P` | `quick_open` | parcial |
| `Cmd+Shift+P` | `command_palette` | parcial. Headless session opens it |
| `Cmd+D` | `select_next_occurrence` | parcial |
| `Cmd+K Cmd+D` | `skip_next_occurrence` | parcial |
| `Cmd+Shift+L` | `split_selection_into_lines` | parcial |
| `Cmd+/` | `toggle_comment` | parcial |
| `Cmd+Alt+Down` | `add_cursor_below` | parcial |
| `Cmd+Alt+Up` | `add_cursor_above` | parcial |
| `Cmd+F` | `find` | parcial. Headless session drives it |
| `Cmd+Alt+F` | `replace` | parcial |
| `Cmd+Shift+F` | `find_in_files` | parcial |
| `Cmd+G` / `Cmd+Shift+G` | `find_next` / `find_prev` | parcial |
| `Cmd+R` / `Cmd+Shift+R` | `goto_symbol` / `goto_symbol_in_project` | parcial |
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

1. **Phase 0, this is the next tanda.** Steps 0.1 and 0.2: encoding
   fixtures and Monokai as the default theme of the whole window, from a
   licensed theme file. Land the compare job and commit
   `monokai-side-by-side.png` and `characters.png`. Block 0 stays open
   because 0.3 is not done. No release.
2. **Phase 0b.** Step 0.3: Retina text and the selectors. Commit
   `selectors.png` and a new Monokai pair. Close block 0 only if Lux holds
   the comparison. That close is the first release this plan allows.
3. **Phase 1.** Block 1, the rest of the window (tree, tabs, status bar,
   minimap, splits, settings) on the Monokai frame from phase 0.
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
