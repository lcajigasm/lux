#!/usr/bin/env bash
# Capture the real Lux and Sublime Text windows on macOS.
# Writes docs/compare/monokai-side-by-side.png and docs/compare/characters.png.
# Does not draw a stand-in if an application cannot be captured.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

out="$root/docs/compare"
fixture="$root/docs/compare/fixtures/monokai.rs"
characters="$root/docs/compare/fixtures/characters.txt"
bin="$root/target/debug/lux"
log="$out/lux.log"
mkdir -p "$out"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "compare captures run on macOS (screencapture). This host is $(uname -s)." >&2
  exit 1
fi

for f in "$fixture" "$characters"; do
  if [[ ! -f "$f" ]]; then
    echo "missing fixture: $f" >&2
    exit 1
  fi
done

echo "--> Building lux (debug, locked)"
cargo build -p lux --locked

cat > /tmp/lux-find-window.swift << 'SWIFT'
import CoreGraphics
import Foundation

let want = CommandLine.arguments.dropFirst().first ?? ""
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else {
    fputs("no window list\n", stderr)
    exit(1)
}
var bestId = 0
var bestArea = 0.0
var best = ""
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 { continue }
    if owner.caseInsensitiveCompare(want) != .orderedSame { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let area = width * height
    if area <= bestArea { continue }
    bestArea = area
    let id = (w[kCGWindowNumber as String] as? NSNumber)?.intValue ?? 0
    let x = (bounds["X"] as? NSNumber)?.intValue ?? 0
    let y = (bounds["Y"] as? NSNumber)?.intValue ?? 0
    bestId = id
    best = "\(id) \(x) \(y) \(Int(width)) \(Int(height))"
}
if bestId == 0 { exit(1) }
print(best)
SWIFT

wait_window() {
  local owner="$1"
  local pid="${2:-}"
  local found=""
  local _
  for _ in $(seq 1 60); do
    if [[ -n "$pid" ]] && ! kill -0 "$pid" 2>/dev/null; then
      echo "$owner process exited before a window appeared" >&2
      return 1
    fi
    if found="$(swift /tmp/lux-find-window.swift "$owner" 2>/dev/null)"; then
      echo "$found"
      return 0
    fi
    sleep 1
  done
  echo "timed out waiting for $owner" >&2
  return 1
}

shoot() {
  local owner="$1"
  local dest="$2"
  local line id bytes
  line="$(swift /tmp/lux-find-window.swift "$owner")"
  id="${line%% *}"
  echo "--> $owner window $line"
  screencapture -l "$id" -o "$dest"
  bytes="$(wc -c < "$dest" | tr -d ' ')"
  if [[ "$bytes" -lt 8000 ]]; then
    echo "capture $dest is only $bytes bytes" >&2
    rm -f "$dest"
    return 1
  fi
}

lux_pid=""
cleanup() {
  if [[ -n "$lux_pid" ]] && kill -0 "$lux_pid" 2>/dev/null; then
    kill "$lux_pid" 2>/dev/null || true
    wait "$lux_pid" 2>/dev/null || true
  fi
  osascript -e 'tell application "Sublime Text" to quit' >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "--> Installing Sublime Text (cask) and ImageMagick"
export HOMEBREW_NO_AUTO_UPDATE=1
export NONINTERACTIVE=1
brew install --cask sublime-text
brew install imagemagick

pref_dir="$HOME/Library/Application Support/Sublime Text/Packages/User"
mkdir -p "$pref_dir"
cat > "$pref_dir/Preferences.sublime-settings" << 'JSON'
{
  "color_scheme": "Monokai.sublime-color-scheme",
  "font_size": 16,
  "font_face": "Menlo",
  "hot_exit": false,
  "remember_open_files": false,
  "update_check": false,
  "ignored_packages": ["Vintage"]
}
JSON

subl="/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl"
if [[ ! -x "$subl" ]]; then
  echo "Sublime Text did not install a subl binary at $subl" >&2
  exit 1
fi

echo "--> Opening the Monokai fixture in Lux and Sublime Text"
"$bin" "$fixture" >"$log" 2>&1 &
lux_pid=$!
"$subl" -n "$fixture" || true

lux_bounds="$(wait_window lux "$lux_pid")" || {
  echo "Lux did not open a window" >&2
  cat "$log" >&2 || true
  exit 1
}
echo "--> lux $lux_bounds"

if ! sublime_bounds="$(wait_window "Sublime Text" "")"; then
  echo "Sublime Text did not open a capturable window. Not writing a stand-in." >&2
  exit 1
fi
echo "--> sublime $sublime_bounds"

osascript << 'APPLESCRIPT' || true
tell application "System Events"
  try
    tell process "lux"
      set position of window 1 to {40, 40}
      set size of window 1 to {1000, 700}
    end tell
  end try
  try
    tell process "Sublime Text"
      set position of window 1 to {40, 40}
      set size of window 1 to {1000, 700}
    end tell
  end try
end tell
APPLESCRIPT
sleep 1

tmp="$(mktemp -d)"
shoot lux "$tmp/lux-monokai.png"
shoot "Sublime Text" "$tmp/sublime-monokai.png"
magick "$tmp/lux-monokai.png" "$tmp/sublime-monokai.png" +append "$out/monokai-side-by-side.png"
echo "--> wrote $out/monokai-side-by-side.png"

kill "$lux_pid" 2>/dev/null || true
wait "$lux_pid" 2>/dev/null || true
lux_pid=""

echo "--> Opening the character fixture in Lux"
"$bin" "$characters" >"$log" 2>&1 &
lux_pid=$!
wait_window lux "$lux_pid" >/dev/null
sleep 1
shoot lux "$out/characters.png"
echo "--> wrote $out/characters.png"
