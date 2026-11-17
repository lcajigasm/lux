#!/usr/bin/env bash
# Launch the compiled Lux.app and capture the Dock with the corner icon,
# then with classic selected. Fails if the probe does not report the switch
# or if either capture is missing.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "app icon shots run on macOS. This host is $(uname -s)." >&2
  exit 1
fi

root="$(git rev-parse --show-toplevel)"
cd "$root"
out="$root/docs/evidence/app-icon"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/*.txt "$out"/*.log

chmod +x "$root/scripts/build-app-icon.sh" "$root/scripts/stage-lux-app.sh"
"$root/scripts/build-app-icon.sh"

checked="$(mktemp -d)"
iconutil -c iconset "$root/packaging/macos/lux.icns" -o "$checked/Lux.iconset"
python3 - "$checked/Lux.iconset" << 'PY'
import struct
import sys
from pathlib import Path

iconset = Path(sys.argv[1])
expect = {
    "icon_16x16.png": 16,
    "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32,
    "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128,
    "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256,
    "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512,
    "icon_512x512@2x.png": 1024,
}
missing = []
for name, side in expect.items():
    path = iconset / name
    if not path.is_file():
        missing.append(name)
        continue
    data = path.read_bytes()
    if data[16:24] != struct.pack(">II", side, side):
        missing.append(f"{name} size")
if missing:
    raise SystemExit("iconutil iconset missing " + ", ".join(missing))
print("iconutil sizes ok")
PY
rm -rf "$checked"

defaults write com.apple.dock autohide -bool false || true
killall Dock >/dev/null 2>&1 || true
sleep 2

app="$(bash "$root/scripts/stage-lux-app.sh")"
test -f "$app/Contents/Resources/lux.icns"
test -f "$app/Contents/Resources/lux-classic.icns"

work="$(mktemp -d)"
home="$(mktemp -d)"
printf 'fn main() {}\n' > "$work/note.rs"
state="$out/state.txt"
pid=""
cleanup() {
  if [[ -n "${pid}" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "--> Launching $app"
HOME="$home" "$app/Contents/MacOS/lux" --probe "$state" "$work/note.rs" >"$out/lux.log" 2>&1 &
pid=$!

for _ in $(seq 1 60); do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "Lux exited before the window appeared" >&2
    cat "$out/lux.log" >&2 || true
    exit 1
  fi
  if swift -e 'import CoreGraphics; import Foundation
let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
for w in info {
  let owner = w[kCGWindowOwnerName as String] as? String ?? ""
  let layer = w[kCGWindowLayer as String] as? Int ?? 0
  if layer != 0 { continue }
  if owner == "Lux" || owner == "lux" { exit(0) }
}
exit(1)' >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

wait_icon() {
  local want="$1"
  local i
  for i in $(seq 1 40); do
    if [[ -f "$state" ]] && grep -q "^icon=${want}$" "$state"; then
      cp "$state" "$out/probe-${want}.txt"
      return 0
    fi
    kill -USR2 "$pid" 2>/dev/null || return 1
    sleep 0.5
  done
  echo "probe did not report icon=${want}" >&2
  cat "$state" >&2 || true
  cat "$out/lux.log" >&2 || true
  return 1
}

capture_dock() {
  local dest="$1"
  local id
  id="$(swift -e 'import CoreGraphics; import Foundation
let info = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
for w in info {
  let owner = w[kCGWindowOwnerName as String] as? String ?? ""
  if owner != "Dock" { continue }
  let num = w[kCGWindowNumber as String] as? Int ?? 0
  let bounds = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
  let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
  let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
  if width < 200 || height < 20 || num == 0 { continue }
  print(num)
  exit(0)
}
exit(1)' 2>/dev/null || true)"
  if [[ -n "$id" ]]; then
    screencapture -x -l "$id" "$dest" || true
  fi
  if [[ ! -s "$dest" ]]; then
    screencapture -x "$out/screen-tmp.png" || true
    if [[ -s "$out/screen-tmp.png" ]]; then
      local h w top
      h="$(sips -g pixelHeight "$out/screen-tmp.png" | awk '/pixelHeight/ {print $2}')"
      w="$(sips -g pixelWidth "$out/screen-tmp.png" | awk '/pixelWidth/ {print $2}')"
      top=0
      if [[ "${h:-0}" -gt 200 && "${w:-0}" -gt 0 ]]; then
        top=$((h - 180))
        sips --cropOffset 0 "$top" --cropToHeightWidth 180 "$w" "$out/screen-tmp.png" --out "$dest" >/dev/null \
          || cp "$out/screen-tmp.png" "$dest"
      else
        cp "$out/screen-tmp.png" "$dest"
      fi
    fi
  fi
  if [[ ! -s "$dest" ]]; then
    echo "no Dock capture at $dest" >&2
    return 1
  fi
}

wait_icon corner
capture_dock "$out/dock-corner.png"

python3 - "$home/.config/lux/settings.json" << 'PY'
import json
import sys
from pathlib import Path
path = Path(sys.argv[1])
data = {}
if path.is_file():
    data = json.loads(path.read_text())
data["workbench.icon"] = "classic"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(data, indent=2) + "\n")
print(f"wrote {path}")
PY

wait_icon classic
sleep 1
capture_dock "$out/dock-classic.png"

if cmp -s "$out/dock-corner.png" "$out/dock-classic.png"; then
  echo "Dock captures are identical, so the icon did not change" >&2
  exit 1
fi
echo "dock captures differ"
ls -l "$out"
