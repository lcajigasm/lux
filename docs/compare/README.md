# Window comparisons

`compare.yml` on standard `macos-15` runs `scripts/compare-captures.sh`.
That script builds Lux, installs Sublime Text with `brew install --cask`,
opens `fixtures/monokai.rs` in both with Monokai, and opens
`fixtures/characters.txt` in Lux. `screencapture -l` records the real
windows. ImageMagick joins the Monokai pair.

Committed PNGs are copies of that job's artifact. A file that did not come
from the job is not a comparison. If Sublime Text cannot be captured, the
job fails and does not write a replacement image.

`selectors.png` is not produced. Phase 0b (icons, rounded corners, motion)
is not done, and Zed is not part of this job.
