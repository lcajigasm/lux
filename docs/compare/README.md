# Window comparisons

`compare.yml` on standard `macos-15` runs `scripts/compare-captures.sh`.
That script builds Lux, installs Sublime Text with `brew install --cask`,
opens `fixtures/monokai.rs` in both with Monokai, and opens
`fixtures/characters.txt` in Lux. `screencapture -l` records the real
windows. ImageMagick joins the Monokai pair.

Committed PNGs, including `goto.png`, are copies of compare run
41357599959. A file that did not come from the job is not a comparison.
If Sublime Text cannot be captured,
the job fails and does not write a replacement image.

`selectors.png` is the same job with a query typed: Lux
(`lux --palette --query sel`), Sublime Text (`show_overlay` with
`text: sel`), and Zed (Cmd-Shift-P, then `sel`). `goto.png` is go to
file with `mon`. Lux opens the fixtures directory for that shot, so the
list is the folder index (`monokai.rs`), not an empty panel from a file
that has not loaded yet. `theme.png` is Lux and Sublime's color-scheme pickers
with `mo`; Zed's pane is its command palette filtered to `theme`,
because this capture has no shortcut that opens Zed's theme selector.
Zed is installed with `brew install --cask zed` and opened
by bundle path (`open -a /Applications/Zed.app`), because `open -a Zed`
does not see a cask Launch Services has not indexed yet. Zed's settings
set `session.trust_all_worktrees`, and the capture also sends Return,
which is that dialog's "Trust and Continue" button. Plain Escape leaves
the dialog up. If Zed or
Sublime cannot be captured, the job fails and does not write a stand-in.
The GitHub macos-15 display has been 1×, so a window screenshot does not
prove Retina. `retina-palette.png` is written by Lux itself: the same
renderer, into an RGBA texture at scale factor 2. That file is real
pixels from the app. It is not an upscale of the 1× screenshot. Sublime
cannot be captured at scale 2 on a 1× framebuffer, and the script does
not invent that pair. HiDPI grayscale is selected when `scale_factor > 1`.
Selector text uses grayscale at scale 1 as well, so it composites on the
panel.
