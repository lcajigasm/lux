#!/usr/bin/env bash
# macos-15: Markdown preview with a rust fence, from the compiled app.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "markdown-shot runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

out="$root/docs/evidence"
mkdir -p "$out"
dest="$out/markdown-fence.png"
rm -f "$dest"

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

fixture="$HOME/fence.md"
cat > "$fixture" << 'EOF'
# Fence

```rust
fn main() {}
```
EOF

drive_launch "$fixture"
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    break
  fi
  sleep 0.25
done
[[ "$(drive_field active_path)" == "$fixture" ]] || {
  echo "fence.md never became active" >&2
  exit 1
}
[[ "$(drive_field language)" == "Markdown" ]] || {
  echo "opened as $(drive_field language)" >&2
  exit 1
}
drive_key tap 35 cmd,shift
sleep 0.35
drive_key chars "toggle preview"
sleep 0.35
drive_key tap 36 none
sleep 0.6
drive_dump
[[ "$(drive_field palette)" == "0" ]] || {
  echo "palette stayed open; preview command did not run" >&2
  exit 1
}
id="$(swift /tmp/lux-window-id.swift)"
screencapture -l "$id" -o "$dest"
[[ -s "$dest" ]] || {
  echo "screencapture produced no markdown-fence.png" >&2
  exit 1
}
echo "PASS $dest"
drive_stop
trap - EXIT
