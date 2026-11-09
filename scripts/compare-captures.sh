#!/usr/bin/env bash
# Capture the real Lux, Sublime Text, and Zed windows on macOS.
# Writes docs/compare/monokai-side-by-side.png, characters.png,
# selectors.png, goto.png, theme.png, tree.png, tabs.png, and settings.png.
# Lux's selector pane is Command-Shift-P and `sel` posted to Lux.app, with
# no --palette flag. retina-palette.png is that same live frame written at
# scale factor 2 when the job sends SIGUSR1. settings.png is still the
# renderer frame from --settings.
# Sublime's preferences surface is the JSON buffer, and this job does not
# open Zed's settings editor, so settings.png is Lux only.
# Does not draw a stand-in if an application cannot be captured.
# A 1× framebuffer cannot screencapture Sublime at scale 2; that file is
# Lux only, and the log says so.
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
lux_app="$(bash "$root/scripts/stage-lux-app.sh" --no-build)"
echo "--> bundled app $lux_app"

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

# `subl` blocks until Sublime's listener socket is up. A first-launch
# license or Gatekeeper dialog never opens that socket, so an unbounded
# `subl` holds the job until the runner kills it. Bound every launch.
bound() {
  local seconds="$1"
  shift
  "$@" &
  local pid=$!
  local i=0
  while (( i < seconds )); do
    if ! kill -0 "$pid" 2>/dev/null; then
      wait "$pid" || true
      return 0
    fi
    sleep 1
    i=$((i + 1))
  done
  echo "--> giving up after ${seconds}s: $*" >&2
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  return 0
}

dump_windows() {
  swift -e 'import CoreGraphics; import Foundation
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] ?? []
for w in info {
  let owner = w[kCGWindowOwnerName as String] as? String ?? ""
  let layer = w[kCGWindowLayer as String] as? Int ?? 0
  if layer != 0 { continue }
  print(owner)
}' 2>/dev/null | sort | uniq -c >&2 || true
}

wait_window() {
  local owner="$1"
  local pid="${2:-}"
  local attempts="${3:-60}"
  local found=""
  local _
  for _ in $(seq 1 "$attempts"); do
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
  echo "timed out waiting for $owner; on-screen owners:" >&2
  dump_windows
  return 1
}

shoot() {
  local owner="$1"
  local dest="$2"
  local attempt line id bytes
  # `screencapture -l` found Zed's window and then exited with no file
  # on run 41427515909 (window 61, 20s). Resolve the id again and retry.
  for attempt in 1 2 3; do
    rm -f "$dest"
    if ! line="$(swift /tmp/lux-find-window.swift "$owner" 2>/dev/null)"; then
      echo "no $owner window on attempt $attempt" >&2
      sleep 1
      continue
    fi
    id="${line%% *}"
    echo "--> $owner window $line (attempt $attempt)"
    bound 20 screencapture -l "$id" -o "$dest"
    if [[ ! -f "$dest" ]]; then
      echo "screencapture produced no file for $owner (attempt $attempt)" >&2
      sleep 1
      continue
    fi
    bytes="$(wc -c < "$dest" | tr -d ' ')"
    if [[ "$bytes" -lt 8000 ]]; then
      echo "capture $dest is only $bytes bytes" >&2
      rm -f "$dest"
      sleep 1
      continue
    fi
    return 0
  done
  return 1
}

lux_pid=""
zed_started=0
zed_bundle_id=""
cleanup() {
  if [[ -n "$lux_pid" ]] && kill -0 "$lux_pid" 2>/dev/null; then
    kill "$lux_pid" 2>/dev/null || true
    wait "$lux_pid" 2>/dev/null || true
  fi
  bound 8 osascript -e 'tell application "Sublime Text" to quit' >/dev/null 2>&1 || true
  if [[ "$zed_started" == 1 ]]; then
    if [[ -n "$zed_bundle_id" ]]; then
      bound 8 osascript -e "tell application id \"$zed_bundle_id\" to quit" >/dev/null 2>&1 || true
    fi
    bound 8 osascript -e 'tell application "Zed" to quit' >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# A raw debug binary has no bundle id. System Events can still raise its process.
front() {
  local name="$1"
  osascript -e "tell application \"System Events\" to set frontmost of process \"$name\" to true" || true
}

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
# A quarantined cask can sit in Gatekeeper verification while `subl` waits.
xattr -dr com.apple.quarantine "/Applications/Sublime Text.app" || true

echo "--> Opening the Monokai fixture in Lux and Sublime Text"
"$bin" "$fixture" >"$log" 2>&1 &
lux_pid=$!
echo "--> lux pid $lux_pid"
# `open` returns once launchd has started the app. `subl` is only used
# after a window exists, so it does not wait on the first-run socket.
bound 30 open -a "Sublime Text" || true

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
echo "--> asking Sublime to open the fixture"
bound 15 "$subl" -n "$fixture" || true
sleep 2
if ! sublime_bounds="$(wait_window "Sublime Text" "")"; then
  echo "Sublime Text lost its window after opening the fixture." >&2
  exit 1
fi

cat > /tmp/lux-bounds.applescript << 'APPLESCRIPT'
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
bound 8 osascript /tmp/lux-bounds.applescript || true
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

kill "$lux_pid" 2>/dev/null || true
wait "$lux_pid" 2>/dev/null || true
lux_pid=""

echo "--> Installing Zed (cask) for the selector capture"
brew install --cask zed
xattr -dr com.apple.quarantine "/Applications/Zed.app" || true
zed_started=1
# A cask installed seconds ago is often missing from Launch Services.
# `open -a Zed` then exits with "Unable to find application named 'Zed'".
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$lsregister" ]]; then
  "$lsregister" -f "/Applications/Zed.app" || true
fi
zed_bundle_id="$(defaults read "/Applications/Zed.app/Contents/Info.plist" CFBundleIdentifier 2>/dev/null || echo "dev.zed.Zed")"
echo "--> Zed bundle id ${zed_bundle_id}"
# Always overwrite. A file Zed created on install would otherwise keep
# the default, and the first open stops on the Unrecognized Project dialog.
for zed_support in \
  "$HOME/Library/Application Support/Zed" \
  "$HOME/.config/zed"
do
  mkdir -p "$zed_support"
  cat > "$zed_support/settings.json" << 'JSON'
{
  "theme": "One Dark",
  "buffer_font_family": "Menlo",
  "buffer_font_size": 15,
  "telemetry": { "diagnostics": false, "metrics": false },
  "session": { "trust_all_worktrees": true }
}
JSON
  echo "--> wrote $zed_support/settings.json"
done

cat > /tmp/lux-keys.swift << 'SWIFT'
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

let escape: CGKeyCode = 0x35
let ret: CGKeyCode = 0x24
let p: CGKeyCode = 0x23
let s: CGKeyCode = 0x01
let e: CGKeyCode = 0x0E
let l: CGKeyCode = 0x25
// The trust dialog's default button is Return ("Trust and Continue").
// Plain Escape does not dismiss it; the label is Option-Escape.
post(ret, true, [])
post(ret, false, [])
Thread.sleep(forTimeInterval: 0.6)
for _ in 0..<3 {
    post(escape, true, [])
    post(escape, false, [])
}
let chord: CGEventFlags = [.maskCommand, .maskShift]
post(p, true, chord)
post(p, false, chord)
Thread.sleep(forTimeInterval: 0.4)
for key in [s, e, l] {
    post(key, true, [])
    post(key, false, [])
    Thread.sleep(forTimeInterval: 0.05)
}
SWIFT

echo "--> Scale-2 settings page from the renderer"
"$bin" --settings --snapshot "$out/settings.png" --snapshot-scale 2 "$fixture" >"$out/settings-snapshot.log" 2>&1 &
settings_pid=$!
settings_ready=0
for _ in $(seq 1 40); do
  if [[ -s "$out/settings.png" ]]; then
    settings_ready=1
    break
  fi
  if ! kill -0 "$settings_pid" 2>/dev/null; then
    break
  fi
  sleep 0.5
done
kill "$settings_pid" 2>/dev/null || true
wait "$settings_pid" 2>/dev/null || true
for _ in $(seq 1 10); do
  if ! swift /tmp/lux-find-window.swift lux >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done
if [[ "$settings_ready" != 1 ]]; then
  echo "Lux did not write settings.png. Not inventing a settings image." >&2
  cat "$out/settings-snapshot.log" >&2 || true
  exit 1
fi
echo "--> Opening Lux.app with no --palette and no --query"
rm -f "$out/retina-palette.png"
"$lux_app/Contents/MacOS/lux" \
  --snapshot-on-usr1 "$out/retina-palette.png" \
  --snapshot-scale 2 \
  "$fixture" >"$log" 2>&1 &
lux_pid=$!
lux_proc=""
for _ in $(seq 1 60); do
  if ! kill -0 "$lux_pid" 2>/dev/null; then
    echo "Lux.app exited before a window appeared" >&2
    cat "$log" >&2 || true
    exit 1
  fi
  for candidate in Lux lux; do
    if swift /tmp/lux-find-window.swift "$candidate" >/dev/null 2>&1; then
      lux_proc="$candidate"
      break
    fi
  done
  if [[ -n "$lux_proc" ]]; then
    break
  fi
  sleep 1
done
if [[ -z "$lux_proc" ]]; then
  echo "Lux.app did not open a window. Not writing selectors.png." >&2
  cat "$log" >&2 || true
  exit 1
fi
echo "--> Lux.app process $lux_proc pid $lux_pid"
bound 15 "$subl" --command 'show_overlay {"overlay": "command_palette", "text": "sel"}' || true
# `bound` returns 0 once the child exits, including when `open` failed.
# Record the real status so a missing app is visible in the log.
launch_zed() {
  if open -a "/Applications/Zed.app" "$fixture"; then
    echo "--> open -a /Applications/Zed.app returned 0"
    return 0
  fi
  echo "--> open -a by bundle path failed; opening the bundle" >&2
  open "/Applications/Zed.app"
}
bound 25 launch_zed || true
if ! wait_window "Zed" "" 90 >/dev/null; then
  echo "Zed did not open a capturable window. Not writing selectors.png." >&2
  dump_windows
  exit 1
fi
sleep 1
if [[ -n "$zed_bundle_id" ]]; then
  bound 8 osascript -e "tell application id \"$zed_bundle_id\" to activate" || true
else
  bound 8 osascript -e 'tell application "Zed" to activate' || true
fi
sleep 1
if ! swift /tmp/lux-keys.swift; then
  echo "Could not send Zed's command-palette shortcut. Not writing selectors.png." >&2
  exit 1
fi
cat > /tmp/lux-own-palette.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
let chord: CGEventFlags = [.maskCommand, .maskShift]
post(0x23, true, chord)
post(0x23, false, chord)
Thread.sleep(forTimeInterval: 0.45)
for key: CGKeyCode in [0x01, 0x0E, 0x25] {
    post(key, true, [])
    post(key, false, [])
    Thread.sleep(forTimeInterval: 0.05)
}
SWIFT
sleep 0.3
if ! osascript -e "tell application \"System Events\" to set frontmost of process \"$lux_proc\" to true" >"$out/lux-focus.err" 2>&1; then
  echo "System Events refused to focus Lux.app (process $lux_proc):" >&2
  cat "$out/lux-focus.err" >&2
  exit 1
fi
sleep 0.4
if ! swift /tmp/lux-own-palette.swift; then
  echo "Could not type Lux's command palette. Not writing selectors.png." >&2
  exit 1
fi
sleep 0.8
kill -USR1 "$lux_pid"
usr1_ready=0
for _ in $(seq 1 40); do
  if [[ -s "$out/retina-palette.png" ]]; then
    usr1_ready=1
    break
  fi
  if ! kill -0 "$lux_pid" 2>/dev/null; then
    break
  fi
  sleep 0.25
done
if [[ "$usr1_ready" != 1 ]]; then
  echo "Lux.app did not write retina-palette.png after the typed palette. Not inventing a scale-2 image." >&2
  tail -40 "$log" >&2 || true
  exit 1
fi

cat > /tmp/lux-selector-bounds.applescript << 'APPLESCRIPT'
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
  try
    tell process "Zed"
      set position of window 1 to {40, 40}
      set size of window 1 to {1000, 700}
    end tell
  end try
end tell
APPLESCRIPT
bound 8 osascript /tmp/lux-selector-bounds.applescript || true
sleep 1

sel="$(mktemp -d)"
shoot "$lux_proc" "$sel/lux.png"
if ! shoot "Sublime Text" "$sel/sublime.png"; then
  echo "Sublime Text's command palette could not be captured. Not writing selectors.png." >&2
  exit 1
fi
if ! shoot "Zed" "$sel/zed.png"; then
  echo "Zed could not be captured. Not writing selectors.png." >&2
  exit 1
fi
magick "$sel/lux.png" "$sel/sublime.png" "$sel/zed.png" +append "$out/selectors.png"
echo "--> wrote $out/selectors.png"
echo "--> lux display scale (1 means this runner is not Retina):"
grep "display scale" "$log" "$out/snapshot.log" || true
if [[ ! -s "$out/retina-palette.png" ]]; then
  echo "Lux did not write retina-palette.png. Not inventing a scale-2 image." >&2
  exit 1
fi
retina_w="$(sips -g pixelWidth "$out/retina-palette.png" | awk '/pixelWidth/ {print $2}')"
retina_h="$(sips -g pixelHeight "$out/retina-palette.png" | awk '/pixelHeight/ {print $2}')"
echo "--> retina-palette.png ${retina_w}x${retina_h}"
if [[ -z "$retina_w" || "$retina_w" -lt 1600 ]]; then
  echo "Scale-2 snapshot is ${retina_w:-0}px wide. Refusing it." >&2
  exit 1
fi
if [[ ! -s "$out/settings.png" ]]; then
  echo "Lux did not write settings.png. Not inventing a settings image." >&2
  exit 1
fi
settings_w="$(sips -g pixelWidth "$out/settings.png" | awk '/pixelWidth/ {print $2}')"
settings_h="$(sips -g pixelHeight "$out/settings.png" | awk '/pixelHeight/ {print $2}')"
echo "--> settings.png ${settings_w}x${settings_h}"
if [[ -z "$settings_w" || "$settings_w" -lt 1600 ]]; then
  echo "Settings snapshot is ${settings_w:-0}px wide. Refusing it." >&2
  exit 1
fi
scale_line="$(grep "display scale" "$log" "$out/snapshot.log" | awk '{print $NF}' | tail -1 || true)"
if [[ "$scale_line" == "1" || "$scale_line" == "1.0" ]]; then
  echo "--> Runner framebuffer is 1×. screencapture cannot record Sublime at scale 2."
  echo "--> retina-palette.png is Lux's renderer at scale factor 2, not an upscaled screenshot."
else
  echo "--> Display scale ${scale_line}. Capturing Sublime's window at the runner's scale."
  if ! shoot "Sublime Text" "$out/retina-sublime.png"; then
    echo "Sublime Text could not be captured at this scale. Not writing a stand-in." >&2
    exit 1
  fi
fi

echo "--> Opening go to file with query mon"
kill "$lux_pid" 2>/dev/null || true
wait "$lux_pid" 2>/dev/null || true
lux_pid=""
# A single file is not a folder index. Open the fixtures directory so
# the panel lists real paths (monokai.rs matches "mon").
"$bin" "$root/docs/compare/fixtures" >"$log" 2>&1 &
lux_pid=$!
wait_window lux "$lux_pid" >/dev/null
bound 15 "$subl" --command 'show_overlay {"overlay": "goto", "text": "mon"}' || true
if [[ -n "$zed_bundle_id" ]]; then
  bound 8 osascript -e "tell application id \"$zed_bundle_id\" to activate" || true
fi
cat > /tmp/lux-goto-keys.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
post(0x35, true, [])
post(0x35, false, [])
Thread.sleep(forTimeInterval: 0.3)
post(0x23, true, [.maskCommand])
post(0x23, false, [.maskCommand])
Thread.sleep(forTimeInterval: 0.3)
for key: CGKeyCode in [0x2E, 0x1F, 0x2D] {
    post(key, true, [])
    post(key, false, [])
    Thread.sleep(forTimeInterval: 0.05)
}
SWIFT
if ! swift /tmp/lux-goto-keys.swift; then
  echo "Could not send Zed's go-to-file shortcut. Not writing goto.png." >&2
  exit 1
fi
cat > /tmp/lux-own-goto.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
post(0x35, true, [])
post(0x35, false, [])
Thread.sleep(forTimeInterval: 0.25)
post(0x23, true, [.maskCommand])
post(0x23, false, [.maskCommand])
Thread.sleep(forTimeInterval: 0.4)
for key: CGKeyCode in [0x2E, 0x1F, 0x2D] {
    post(key, true, [])
    post(key, false, [])
    Thread.sleep(forTimeInterval: 0.05)
}
SWIFT
front lux
sleep 0.4
if ! swift /tmp/lux-own-goto.swift; then
  echo "Could not type Lux's go-to panel. Not writing goto.png." >&2
  exit 1
fi
sleep 0.5
bound 8 osascript /tmp/lux-selector-bounds.applescript || true
sleep 1
goto_dir="$(mktemp -d)"
shoot lux "$goto_dir/lux.png"
if ! shoot "Sublime Text" "$goto_dir/sublime.png"; then
  echo "Sublime Text's go-to panel could not be captured. Not writing goto.png." >&2
  exit 1
fi
if ! shoot "Zed" "$goto_dir/zed.png"; then
  echo "Zed's go-to panel could not be captured. Not writing goto.png." >&2
  exit 1
fi
magick "$goto_dir/lux.png" "$goto_dir/sublime.png" "$goto_dir/zed.png" +append "$out/goto.png"
echo "--> wrote $out/goto.png"

echo "--> Opening the color scheme picker with query mo"
kill "$lux_pid" 2>/dev/null || true
wait "$lux_pid" 2>/dev/null || true
lux_pid=""
"$bin" "$fixture" >"$log" 2>&1 &
lux_pid=$!
wait_window lux "$lux_pid" >/dev/null
bound 15 "$subl" --command "select_color_scheme" || true
sleep 0.4
# "mo" into whichever panel Sublime opened. Zed has no stable theme-picker
# shortcut here; its pane is the command palette filtered to "theme".
cat > /tmp/lux-theme-keys.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
for key: CGKeyCode in [0x2E, 0x1F] {
    post(key, true, [])
    post(key, false, [])
    Thread.sleep(forTimeInterval: 0.05)
}
SWIFT
bound 8 osascript -e 'tell application "Sublime Text" to activate' || true
sleep 0.3
swift /tmp/lux-theme-keys.swift || true
if [[ -n "$zed_bundle_id" ]]; then
  bound 8 osascript -e "tell application id \"$zed_bundle_id\" to activate" || true
fi
sleep 0.2
cat > /tmp/lux-zed-theme.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
post(0x35, true, [])
post(0x35, false, [])
Thread.sleep(forTimeInterval: 0.2)
let chord: CGEventFlags = [.maskCommand, .maskShift]
post(0x23, true, chord)
post(0x23, false, chord)
Thread.sleep(forTimeInterval: 0.3)
// t h e m e
for key: CGKeyCode in [0x11, 0x04, 0x0E, 0x2E, 0x0E] {
    post(key, true, [])
    post(key, false, [])
    Thread.sleep(forTimeInterval: 0.05)
}
SWIFT
if ! swift /tmp/lux-zed-theme.swift; then
  echo "Could not filter Zed's palette to theme. Not writing theme.png." >&2
  exit 1
fi
cat > /tmp/lux-own-theme.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
func tap(_ key: CGKeyCode, _ flags: CGEventFlags) {
    post(key, true, flags)
    post(key, false, flags)
    Thread.sleep(forTimeInterval: 0.05)
}
post(0x35, true, [])
post(0x35, false, [])
Thread.sleep(forTimeInterval: 0.25)
let chord: CGEventFlags = [.maskCommand, .maskShift]
post(0x23, true, chord)
post(0x23, false, chord)
Thread.sleep(forTimeInterval: 0.4)
// "select color scheme" — the title of UI: Select Color Scheme.
for key: CGKeyCode in [0x01, 0x0E, 0x25, 0x0E, 0x08, 0x11, 0x31, 0x08, 0x1F, 0x25, 0x0F, 0x31, 0x01, 0x08, 0x04, 0x0E, 0x2E, 0x0E] {
    tap(key, [])
}
Thread.sleep(forTimeInterval: 0.2)
post(0x24, true, [])
post(0x24, false, [])
Thread.sleep(forTimeInterval: 0.45)
tap(0x2E, [])
tap(0x1F, [])
SWIFT
front lux
sleep 0.4
if ! swift /tmp/lux-own-theme.swift; then
  echo "Could not type Lux's theme picker. Not writing theme.png." >&2
  exit 1
fi
sleep 0.5
bound 8 osascript /tmp/lux-selector-bounds.applescript || true
sleep 1
theme_dir="$(mktemp -d)"
shoot lux "$theme_dir/lux.png"
if ! shoot "Sublime Text" "$theme_dir/sublime.png"; then
  echo "Sublime Text's theme picker could not be captured. Not writing theme.png." >&2
  exit 1
fi
if ! shoot "Zed" "$theme_dir/zed.png"; then
  echo "Zed could not be captured. Not writing theme.png." >&2
  exit 1
fi
magick "$theme_dir/lux.png" "$theme_dir/sublime.png" "$theme_dir/zed.png" +append "$out/theme.png"
echo "--> wrote $out/theme.png"
echo "--> Zed's theme pane is the command palette filtered to theme."
echo "--> This runner has no shortcut that opens Zed's theme selector itself."

wait_for_log() {
  local needle="$1"
  local _
  for _ in $(seq 1 40); do
    if grep -q "$needle" "$log"; then
      return 0
    fi
    sleep 0.25
  done
  echo "lux log never contained: $needle" >&2
  cat "$log" >&2 || true
  return 1
}

# A new window at 1100×760 is the largest for that app, so screencapture
# records it rather than an earlier 1000×700 window.
cat > /tmp/lux-phase1-bounds.applescript << 'APPLESCRIPT'
tell application "System Events"
  try
    tell process "lux"
      set position of window 1 to {40, 40}
      set size of window 1 to {1100, 760}
    end tell
  end try
  try
    tell process "Sublime Text"
      set position of window 1 to {40, 40}
      set size of window 1 to {1100, 760}
    end tell
  end try
  try
    tell process "Zed"
      set position of window 1 to {40, 40}
      set size of window 1 to {1100, 760}
    end tell
  end try
end tell
APPLESCRIPT

echo "--> Opening the fixtures folder so the file tree is on screen"
kill "$lux_pid" 2>/dev/null || true
wait "$lux_pid" 2>/dev/null || true
lux_pid=""
"$bin" "$root/docs/compare/fixtures" >"$log" 2>&1 &
lux_pid=$!
wait_window lux "$lux_pid" >/dev/null
bound 15 "$subl" -n "$root/docs/compare/fixtures" || true
bound 20 open -a "/Applications/Zed.app" "$root/docs/compare/fixtures" || true
if ! wait_window "Zed" "" 30 >/dev/null; then
  echo "Zed did not open the fixtures folder. Not writing tree.png." >&2
  exit 1
fi
sleep 1
bound 8 osascript /tmp/lux-phase1-bounds.applescript || true
sleep 1
tree_dir="$(mktemp -d)"
shoot lux "$tree_dir/lux.png"
if ! shoot "Sublime Text" "$tree_dir/sublime.png"; then
  echo "Sublime Text's folder window could not be captured. Not writing tree.png." >&2
  exit 1
fi
if ! shoot "Zed" "$tree_dir/zed.png"; then
  echo "Zed's folder window could not be captured. Not writing tree.png." >&2
  exit 1
fi
magick "$tree_dir/lux.png" "$tree_dir/sublime.png" "$tree_dir/zed.png" +append "$out/tree.png"
echo "--> wrote $out/tree.png"

echo "--> Opening two files so the tab strip shows an active and an inactive tab"
kill "$lux_pid" 2>/dev/null || true
wait "$lux_pid" 2>/dev/null || true
lux_pid=""
"$bin" "$fixture" "$characters" >"$log" 2>&1 &
lux_pid=$!
wait_window lux "$lux_pid" >/dev/null
wait_for_log "monokai.rs"
wait_for_log "characters.txt"
bound 15 "$subl" -n "$fixture" "$characters" || true
if [[ -x "/Applications/Zed.app/Contents/MacOS/cli" ]]; then
  bound 20 "/Applications/Zed.app/Contents/MacOS/cli" "$fixture" "$characters" || true
else
  bound 20 open -a "/Applications/Zed.app" "$fixture" "$characters" || true
fi
sleep 1
bound 8 osascript /tmp/lux-phase1-bounds.applescript || true
sleep 1
tabs_dir="$(mktemp -d)"
shoot lux "$tabs_dir/lux.png"
if ! shoot "Sublime Text" "$tabs_dir/sublime.png"; then
  echo "Sublime Text's tabs could not be captured. Not writing tabs.png." >&2
  exit 1
fi
if ! shoot "Zed" "$tabs_dir/zed.png"; then
  echo "Zed's tabs could not be captured. Not writing tabs.png." >&2
  exit 1
fi
magick "$tabs_dir/lux.png" "$tabs_dir/sublime.png" "$tabs_dir/zed.png" +append "$out/tabs.png"
echo "--> wrote $out/tabs.png"
