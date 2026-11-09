#!/usr/bin/env bash
# Smoke the bundled Lux.app with real key events on macOS, then ask the
# running process for scale-2 frames of what it is showing. Fails if the
# saved file does not contain the typed token, if the process dies, or if
# System Events / CGEvent reports an error. Does not draw a stand-in image.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "evidence smoke runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

root="$(git rev-parse --show-toplevel)"
cd "$root"
out="$root/docs/evidence"
mkdir -p "$out"
rm -f "$out/editor.png" "$out/palette.png" "$out/settings.png" "$out/smoke.log"

app="$(bash "$root/scripts/stage-lux-app.sh")"
work="$(mktemp -d)"
file="$work/evidence.rs"
token="LuxEvidence42"
printf 'fn main() {\n}\n' > "$file"
log="$out/smoke.log"
pid=""

cleanup() {
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "--> Launching $app"
"$app/Contents/MacOS/lux" \
  --snapshot-on-usr1 "$out/editor.png" \
  --snapshot-on-usr1 "$out/palette.png" \
  --snapshot-on-usr1 "$out/settings.png" \
  --snapshot-scale 2 \
  "$file" >"$log" 2>&1 &
pid=$!
echo "--> lux pid $pid"

cat > /tmp/lux-find-window.swift << 'SWIFT'
import CoreGraphics
import Foundation
let want = CommandLine.arguments.dropFirst().first ?? ""
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else {
    fputs("no window list\n", stderr)
    exit(1)
}
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 { continue }
    if owner.caseInsensitiveCompare(want) != .orderedSame { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    exit(0)
}
exit(1)
SWIFT

owner=""
for _ in $(seq 1 60); do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "Lux.app exited before a window appeared" >&2
    cat "$log" >&2 || true
    exit 1
  fi
  for candidate in Lux lux; do
    if swift /tmp/lux-find-window.swift "$candidate" >/dev/null 2>&1; then
      owner="$candidate"
      break
    fi
  done
  if [[ -n "$owner" ]]; then
    break
  fi
  sleep 1
done
if [[ -z "$owner" ]]; then
  echo "timed out waiting for Lux.app; smoke log:" >&2
  cat "$log" >&2 || true
  exit 1
fi
echo "--> window owner $owner"

focus() {
  local err="$out/focus.err"
  if ! osascript -e "tell application \"System Events\" to set frontmost of process \"$owner\" to true" >"$err" 2>&1; then
    echo "System Events refused to focus Lux.app (process $owner):" >&2
    cat "$err" >&2
    exit 1
  fi
}

cat > /tmp/lux-evidence-keys.swift << 'SWIFT'
import CoreGraphics
import Foundation

func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else {
        fputs("cannot build a key event\n", stderr)
        exit(1)
    }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}

func type(_ text: String) {
    let units = Array(text.utf16)
    let source = CGEventSource(stateID: .hidSystemState)
    guard let down = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true),
          let up = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false) else {
        fputs("cannot build a text event\n", stderr)
        exit(1)
    }
    down.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
    up.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
    down.flags = []
    up.flags = []
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
}

func chord(_ key: CGKeyCode, _ flags: CGEventFlags) {
    post(key, true, flags)
    post(key, false, flags)
}

let command = CGEventFlags.maskCommand
let commandShift: CGEventFlags = [.maskCommand, .maskShift]
let step = CommandLine.arguments.dropFirst().first ?? ""
switch step {
case "type":
    type("LuxEvidence42")
case "goto":
    chord(0x23, command) // Cmd+P
    Thread.sleep(forTimeInterval: 0.4)
    chord(0x35, []) // Escape
case "palette":
    chord(0x23, commandShift) // Cmd+Shift+P
    Thread.sleep(forTimeInterval: 0.4)
    for ch in ["s", "e", "l"] {
        type(ch)
        Thread.sleep(forTimeInterval: 0.05)
    }
case "select":
    chord(0x02, command) // Cmd+D
case "settings":
    chord(0x2B, command) // Cmd+,
case "scroll":
    let source = CGEventSource(stateID: .hidSystemState)
    guard let wheel = CGEvent(
        scrollWheelEvent2Source: source,
        units: .pixel,
        wheelCount: 1,
        wheel1: -240,
        wheel2: 0,
        wheel3: 0
    ) else {
        fputs("cannot build a scroll event\n", stderr)
        exit(1)
    }
    wheel.post(tap: .cghidEventTap)
case "save":
    chord(0x01, command) // Cmd+S
case "escape":
    chord(0x35, [])
default:
    fputs("unknown step\n", stderr)
    exit(2)
}
SWIFT

keys() {
  local step="$1"
  focus
  if ! swift /tmp/lux-evidence-keys.swift "$step"; then
    echo "CGEvent step '$step' failed. System Events / event tap output is above." >&2
    exit 1
  fi
}

wait_png() {
  local path="$1"
  local _
  for _ in $(seq 1 40); do
    if [[ -s "$path" ]]; then
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "Lux.app exited while waiting for $path" >&2
      cat "$log" >&2 || true
      exit 1
    fi
    sleep 0.25
  done
  echo "timed out waiting for $path" >&2
  tail -40 "$log" >&2 || true
  exit 1
}

snap() {
  local path="$1"
  kill -USR1 "$pid"
  wait_png "$path"
  local width
  width="$(sips -g pixelWidth "$path" | awk '/pixelWidth/ {print $2}')"
  echo "--> $path ${width}px"
  if [[ -z "$width" || "$width" -lt 1600 ]]; then
    echo "scale-2 frame $path is ${width:-0}px wide" >&2
    exit 1
  fi
}

echo "--> typing $token"
keys type
sleep 0.6
snap "$out/editor.png"

echo "--> Command-P"
keys goto
sleep 0.3

echo "--> Command-Shift-P, then sel"
keys palette
sleep 0.8
snap "$out/palette.png"
keys escape
sleep 0.3

echo "--> Command-D"
keys select
sleep 0.2

echo "--> Command-Comma (Settings)"
keys settings
sleep 0.6
snap "$out/settings.png"

echo "--> scroll, leave Settings, then Command-S"
keys scroll
sleep 0.2
keys escape
sleep 0.3
keys save
sleep 0.8

if ! kill -0 "$pid" 2>/dev/null; then
  echo "Lux.app crashed before the save check" >&2
  cat "$log" >&2 || true
  exit 1
fi
if ! grep -q "$token" "$file"; then
  echo "saved file does not contain $token" >&2
  echo "----- $file -----" >&2
  cat "$file" >&2 || true
  echo "----- smoke log -----" >&2
  tail -60 "$log" >&2 || true
  exit 1
fi
if cmp -s "$out/editor.png" "$out/palette.png"; then
  echo "palette frame is identical to the editor frame" >&2
  exit 1
fi
if cmp -s "$out/editor.png" "$out/settings.png"; then
  echo "settings frame is identical to the editor frame" >&2
  exit 1
fi
echo "--> saved file contains $token and Lux.app is still running"
