# Window comparisons

`compare.yml` on standard `macos-15` runs `scripts/compare-captures.sh`.
That script builds Lux, installs Sublime Text with `brew install --cask`,
opens `fixtures/monokai.rs` in both with Monokai, and opens
`fixtures/characters.txt` in Lux. `screencapture -l` records the real
windows. ImageMagick joins the Monokai pair.

Committed PNGs are copies of compare run 41338114570. A file that did not
come from the job is not a comparison. If Sublime Text cannot be captured,
the job fails and does not write a replacement image.

`selectors.png` is the same job with the command palette open: Lux
(`lux --palette`), Sublime Text (`show_overlay` command palette), and Zed
(Cmd-Shift-P). Zed is installed with `brew install --cask zed`. If Zed or
Sublime cannot be captured, the job fails and does not write a stand-in.
The GitHub macos-15 display is 1×, so this file does not prove Retina
pixels. HiDPI grayscale is selected when `scale_factor > 1` and is covered
by `rendering_mode_for_scale`.
