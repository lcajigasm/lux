#!/usr/bin/env bash
# macos-15: the compiled app after the first frame, with the coverage
# face logged beside the window capture.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "h1-shot runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

out="$root/docs/evidence"
mkdir -p "$out"
dest="$out/h1-editor-font.png"
phases="$out/h1-phases.txt"
rm -f "$dest" "$phases"

cat > /tmp/lux-window-id.swift << 'SWIFT'
import CoreGraphics
import Foundation
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 { continue }
    if owner.caseInsensitiveCompare("Lux") != .orderedSame && owner.caseInsensitiveCompare("lux") != .orderedSame {
        continue
    }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let id = w[kCGWindowNumber as String] as? Int ?? 0
    print("\(id)")
    exit(0)
}
exit(1)
SWIFT

app="$(bash "$root/scripts/stage-lux-app.sh")"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
drive_init "$app"
trap drive_stop EXIT

fixture="$HOME/h1-sample.rs"
cat > "$fixture" << 'EOF'
fn greet() {
    // Coverage faces load after the first frame.
    let label = "Hello 日本語";
}
EOF

drive_launch "$fixture"
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    break
  fi
  sleep 0.25
done
[[ "$(drive_field active_path)" == "$fixture" ]] || {
  echo "sample never became active" >&2
  exit 1
}
# The first frame is already up. Give the coverage pass one redraw.
sleep 0.8
grep -E 'Lux: (phase|first frame|using system font)' "$DRIVE_LOG" > "$phases" || true
id="$(swift /tmp/lux-window-id.swift)"
screencapture -l "$id" -o "$dest"
[[ -s "$dest" ]] || {
  echo "screencapture produced no h1-editor-font.png" >&2
  exit 1
}
echo "PASS $dest"
if [[ -s "$phases" ]]; then
  echo "PASS $phases"
  cat "$phases"
fi
drive_stop
trap - EXIT
