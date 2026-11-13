#!/usr/bin/env bash
# Real Lux.app on macOS: drag a file from Finder onto the window and open it.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/file-drop"
mkdir -p "$out"
rm -f "$out"/*.png

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

drop_dir="$HOME/inbox"
mkdir -p "$drop_dir"
starter="$HOME/starter.txt"
printf 'starter\n' > "$starter"
printf 'dropped-from-finder\n' > "$drop_dir/drop-me.txt"

drive_init "$root/target/Lux.app"
drive_launch "$starter" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/dropped.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == *starter.txt ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "starter.txt never became active" >&2
  cat "$DRIVE_PROBE" >&2 || true
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi

osascript << EOF
tell application "System Events"
  tell process "$DRIVE_OWNER"
    set position of window 1 to {480, 80}
    set size of window 1 to {640, 480}
  end tell
end tell
tell application "Finder"
  activate
  set targetFolder to POSIX file "$drop_dir" as alias
  set finderWindow to make new Finder window to targetFolder
  set current view of finderWindow to icon view
  set bounds of finderWindow to {40, 80, 420, 520}
  select file "drop-me.txt" of targetFolder
end tell
EOF
sleep 0.6

read -r lux_x lux_y lux_w lux_h <<<"$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
drop_x=$((lux_x + lux_w / 2))
drop_y=$((lux_y + lux_h / 2))

drag_file() {
  local x1="$1" y1="$2" x2="$3" y2="$4"
  X1="$x1" Y1="$y1" X2="$x2" Y2="$y2" swift - << 'SWIFT'
import CoreGraphics
import Foundation
let env = ProcessInfo.processInfo.environment
guard let x1 = Double(env["X1"] ?? ""),
      let y1 = Double(env["Y1"] ?? ""),
      let x2 = Double(env["X2"] ?? ""),
      let y2 = Double(env["Y2"] ?? "") else { exit(2) }
let source = CGEventSource(stateID: .hidSystemState)
let start = CGPoint(x: x1, y: y1)
let end = CGPoint(x: x2, y: y2)
if let move = CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: start, mouseButton: .left) {
    move.post(tap: .cghidEventTap)
}
Thread.sleep(forTimeInterval: 0.05)
guard let down = CGEvent(mouseEventSource: source, mouseType: .leftMouseDown, mouseCursorPosition: start, mouseButton: .left) else { exit(1) }
down.post(tap: .cghidEventTap)
let steps = 12
for step in 1...steps {
    let x = x1 + (x2 - x1) * Double(step) / Double(steps)
    let y = y1 + (y2 - y1) * Double(step) / Double(steps)
    let point = CGPoint(x: x, y: y)
    Thread.sleep(forTimeInterval: 0.02)
    if let drag = CGEvent(mouseEventSource: source, mouseType: .leftMouseDragged, mouseCursorPosition: point, mouseButton: .left) {
        drag.post(tap: .cghidEventTap)
    }
}
Thread.sleep(forTimeInterval: 0.05)
guard let up = CGEvent(mouseEventSource: source, mouseType: .leftMouseUp, mouseCursorPosition: end, mouseButton: .left) else { exit(1) }
up.post(tap: .cghidEventTap)
SWIFT
}

landed=0
for y in 150 190 230 270 320 370; do
  for x in 90 150 210 280 340; do
    drag_file "$x" "$y" "$drop_x" "$drop_y" || true
    sleep 0.4
    drive_dump || true
    path="$(drive_field active_path || true)"
    text="$(drive_field text || true)"
    if [[ "$path" == *drop-me.txt && "$text" == *dropped-from-finder* ]]; then
      landed=1
      echo "dropped at finder point $x $y onto $drop_x $drop_y"
      break 2
    fi
  done
done
if [[ "$landed" != 1 ]]; then
  echo "Finder drag did not open drop-me.txt" >&2
  cat "$DRIVE_PROBE" >&2 || true
  tail -40 "$DRIVE_LOG" >&2 || true
  exit 1
fi

drive_focus
sleep 0.3
rm -f "$out/dropped.png"
kill -USR1 "$DRIVE_PID"
for _ in $(seq 1 40); do
  if [[ -s "$out/dropped.png" ]]; then
    break
  fi
  sleep 0.25
done
if [[ ! -s "$out/dropped.png" ]]; then
  echo "snapshot missing" >&2
  exit 1
fi
drive_stop
osascript -e 'tell application "Finder" to close every window' >/dev/null 2>&1 || true
echo "file drop capture written"
