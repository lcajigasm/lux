#!/usr/bin/env bash
# macos-15: one renderer PNG per bundled theme, then a real-key check that
# the theme selector previews the next theme and Escape restores the previous one.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
out="$root/docs/evidence/themes"
mkdir -p "$out"
report="$out/report.txt"
: > "$report"
fail() { echo "FAIL $*" | tee -a "$report" >&2; exit 1; }

app="$(bash "$root/scripts/stage-lux-app.sh")"
bin="$app/Contents/MacOS/lux"
work="$(mktemp -d)"
sample="$work/sample.rs"
cat > "$sample" << 'RS'
fn main() {
    let message = "theme";
    // comment
    println!("{}", 42);
}
RS

cat > /tmp/lux-theme-pixel.swift << 'SWIFT'
import AppKit
let path = CommandLine.arguments[1]
guard let img = NSImage(contentsOfFile: path),
      let tiff = img.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff) else {
    fputs("unreadable png\n", stderr)
    exit(1)
}
let x = rep.pixelsWide - 24
let y = 80
guard x > 0, y < rep.pixelsHigh, let color = rep.colorAt(x: x, y: y) else {
    fputs("pixel out of range\n", stderr)
    exit(1)
}
let rgb = color.usingColorSpace(.sRGB) ?? color
let r = Int((rgb.redComponent * 255.0).rounded())
let g = Int((rgb.greenComponent * 255.0).rounded())
let b = Int((rgb.blueComponent * 255.0).rounded())
print(String(format: "%02x%02x%02x", r, g, b))
SWIFT

pixel() {
  swift /tmp/lux-theme-pixel.swift "$1"
}

snap_theme() {
  local name="$1"
  local slug="$2"
  local home="$work/home-$slug"
  local png="$out/$slug.png"
  rm -rf "$home" "$png"
  mkdir -p "$home/.config/lux"
  printf '{\n  "workbench.colorTheme": "%s"\n}\n' "$name" > "$home/.config/lux/settings.json"
  HOME="$home" "$bin" --snapshot "$png" --snapshot-scale 2 "$sample" >"$work/$slug.log" 2>&1 &
  local pid=$!
  local _
  # The PNG is written after the file is open and the frame has settled.
  # A busy macos-15 runner can take longer than a few seconds; the process
  # is still working. Stop early only when it exits. 45s is the hang cap.
  for _ in $(seq 1 180); do
    if [[ -s "$png" ]]; then
      break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      break
    fi
    sleep 0.25
  done
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  if [[ ! -s "$png" ]]; then
    cat "$work/$slug.log" >&2 || true
    fail "no capture for $name"
  fi
  local color
  color="$(pixel "$png")"
  echo "$slug $name $color" | tee -a "$report"
  case " $colors " in
    *" $color "*) fail "$name pixel $color repeats an earlier theme" ;;
  esac
  colors="$colors $color"
}

colors=""
while IFS='|' read -r name slug; do
  [[ -z "$name" ]] && continue
  snap_theme "$name" "$slug"
done << 'THEMES'
Monokai|monokai
One Dark|one-dark
Dracula|dracula
Solarized Dark|solarized-dark
Solarized Light|solarized-light
Nord|nord
Gruvbox|gruvbox
Tokyo Night|tokyo-night
Catppuccin|catppuccin
Catppuccin Latte|catppuccin-latte
GitHub Dark|github-dark
GitHub Light|github-light
Ayu|ayu
Rose Pine|rose-pine
THEMES

cat > /tmp/lux-theme-keys.swift << 'SWIFT'
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

func hold(_ flags: CGEventFlags, _ down: Bool) {
    let keys: [(CGKeyCode, CGEventFlags)] = [
        (55, .maskCommand),
        (56, .maskShift),
        (58, .maskAlternate),
        (59, .maskControl),
    ]
    let order = down ? keys : keys.reversed()
    for (code, flag) in order where flags.contains(flag) {
        post(code, down, [])
    }
}

func tap(_ key: CGKeyCode, _ flags: CGEventFlags) {
    hold(flags, true)
    if flags.rawValue != 0 { Thread.sleep(forTimeInterval: 0.05) }
    post(key, true, flags)
    Thread.sleep(forTimeInterval: 0.03)
    post(key, false, flags)
    hold(flags, false)
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

let commandShift: CGEventFlags = [.maskCommand, .maskShift]
switch CommandLine.arguments.dropFirst().first ?? "" {
case "open-selector":
    tap(35, commandShift) // Command-Shift-P
    Thread.sleep(forTimeInterval: 0.4)
    for ch in "Select Color Scheme" {
        type(String(ch))
        Thread.sleep(forTimeInterval: 0.03)
    }
    Thread.sleep(forTimeInterval: 0.2)
    tap(36, []) // Return
    Thread.sleep(forTimeInterval: 0.3)
    tap(125, []) // Arrow down
case "escape":
    tap(53, [])
default:
    fputs("unknown step\n", stderr)
    exit(2)
}
SWIFT

cat > /tmp/lux-theme-window.swift << 'SWIFT'
import CoreGraphics
import Foundation
let want = CommandLine.arguments.dropFirst().first ?? ""
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 || owner.caseInsensitiveCompare(want) != .orderedSame { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    exit(0)
}
exit(1)
SWIFT

home="$work/home-keys"
mkdir -p "$home/.config/lux"
printf '{\n  "workbench.colorTheme": "monokai"\n}\n' > "$home/.config/lux/settings.json"
rm -f "$out/selector-before.png" "$out/selector-preview.png" "$out/selector-after.png"
HOME="$home" "$bin" \
  --snapshot-on-usr1 "$out/selector-before.png" \
  --snapshot-on-usr1 "$out/selector-preview.png" \
  --snapshot-on-usr1 "$out/selector-after.png" \
  --snapshot-scale 2 \
  "$sample" >"$work/keys.log" 2>&1 &
pid=$!
owner=""
for _ in $(seq 1 60); do
  if ! kill -0 "$pid" 2>/dev/null; then
    cat "$work/keys.log" >&2 || true
    fail "Lux exited before the selector test"
  fi
  for candidate in Lux lux; do
    if swift /tmp/lux-theme-window.swift "$candidate" >/dev/null 2>&1; then
      owner="$candidate"
      break
    fi
  done
  [[ -n "$owner" ]] && break
  sleep 1
done
[[ -n "$owner" ]] || fail "no Lux window for the selector test"

focus() {
  local err="$work/focus.err"
  if ! osascript -e "tell application \"System Events\" to set frontmost of process \"$owner\" to true" >"$err" 2>&1; then
    cat "$err" >&2
    fail "System Events refused to focus Lux"
  fi
}

wait_png() {
  local path="$1"
  local _
  for _ in $(seq 1 60); do
    if [[ -s "$path" ]]; then
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      cat "$work/keys.log" >&2 || true
      fail "Lux exited waiting for $path"
    fi
    sleep 0.25
  done
  tail -40 "$work/keys.log" >&2 || true
  fail "timed out waiting for $path"
}

snap() {
  kill -USR1 "$pid"
  wait_png "$1"
}

focus
sleep 0.4
snap "$out/selector-before.png"
focus
swift /tmp/lux-theme-keys.swift open-selector
sleep 0.6
snap "$out/selector-preview.png"
focus
swift /tmp/lux-theme-keys.swift escape
sleep 0.4
snap "$out/selector-after.png"
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true

before="$(pixel "$out/selector-before.png")"
preview="$(pixel "$out/selector-preview.png")"
after="$(pixel "$out/selector-after.png")"
echo "selector before=$before preview=$preview after=$after" | tee -a "$report"
if [[ "$preview" == "$before" ]]; then
  fail "selector preview did not change the theme pixel"
fi
if [[ "$after" != "$before" ]]; then
  fail "Escape left pixel $after, started at $before"
fi
echo "PASS themes and selector" | tee -a "$report"
