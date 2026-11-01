#!/usr/bin/env bash
# Capture the real Lux, Sublime Text, and Zed windows on macOS.
# Writes docs/compare/monokai-side-by-side.png, characters.png, and
# selectors.png. Does not draw a stand-in if an application cannot be captured.
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
  local line id bytes
  line="$(swift /tmp/lux-find-window.swift "$owner")"
  id="${line%% *}"
  echo "--> $owner window $line"
  bound 20 screencapture -l "$id" -o "$dest"
  if [[ ! -f "$dest" ]]; then
    echo "screencapture produced no file for $owner" >&2
    return 1
  fi
  bytes="$(wc -c < "$dest" | tr -d ' ')"
  if [[ "$bytes" -lt 8000 ]]; then
    echo "capture $dest is only $bytes bytes" >&2
    rm -f "$dest"
    return 1
  fi
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
SWIFT

echo "--> Opening command palettes in Lux, Sublime Text, and Zed"
"$bin" --palette "$fixture" >"$log" 2>&1 &
lux_pid=$!
wait_window lux "$lux_pid" >/dev/null
bound 15 "$subl" --command 'show_overlay {"overlay": "command_palette"}' || true
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
sleep 1

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
shoot lux "$sel/lux.png"
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
grep "display scale" "$log" || true
