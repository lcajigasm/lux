#!/usr/bin/env bash
# macos-15: conflict markers, then Accept Current from the caret.
# Both PNGs are screencaptures of the compiled window.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "conflict-shot runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

out="$root/docs/evidence"
mkdir -p "$out"
before="$out/conflict-markers.png"
after="$out/conflict-accepted.png"
rm -f "$before" "$after"

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

shoot() {
  local dest="$1"
  local id
  id="$(swift /tmp/lux-window-id.swift)"
  screencapture -l "$id" -o "$dest"
  [[ -s "$dest" ]] || {
    echo "screencapture produced no file at $dest" >&2
    exit 1
  }
}

app="$(bash "$root/scripts/stage-lux-app.sh")"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
drive_init "$app"
trap drive_stop EXIT

fixture="$HOME/conflict.rs"
cat > "$fixture" << 'EOF'
fn main() {
<<<<<<< HEAD
    ours();
=======
    theirs();
>>>>>>> topic
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
  echo "conflict.rs never became active" >&2
  exit 1
}
drive_key tap 125 none
sleep 0.3
drive_dump
[[ "$(drive_field text)" == *'<<<<<<<'* ]] || {
  echo "buffer lost the conflict markers" >&2
  exit 1
}
shoot "$before"
drive_key tap 35 cmd,shift
sleep 0.35
drive_key chars "accept current"
sleep 0.35
drive_key tap 36 none

resolved=0
for _ in $(seq 1 40); do
  drive_dump
  text="$(drive_field text)"
  if [[ "$text" != *'<<<<<<<'* && "$text" == *ours* && "$text" != *theirs* ]]; then
    resolved=1
    break
  fi
  sleep 0.25
done
[[ "$resolved" == 1 ]] || {
  echo "Accept Current left the markers" >&2
  drive_dump || true
  echo "text=$(drive_field text)" >&2
  exit 1
}
sleep 0.3
shoot "$after"
echo "PASS $before"
echo "PASS $after"
drive_stop
trap - EXIT
