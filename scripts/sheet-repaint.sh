#!/usr/bin/env bash
# Cmd+O on Lux.app opens NSOpenPanel as a sheet on the editor window, and
# the editor keeps running its event loop and painting behind it.
#
# Fails when:
#   - no sheet is attached to the Lux window after Cmd+O (an app-modal
#     panel or nothing at all);
#   - the probe cannot be read while the sheet is up (the main thread is
#     stuck in a nested modal loop and never reaches `about_to_wait`);
#   - the redraw counter does not advance while the sheet is up (the caret
#     blink is the only thing asking for frames);
#   - an external edit to the open file does not reach the buffer and the
#     window pixels while the sheet is up;
#   - after Escape closes the sheet, typed text does not land in the editor.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/sheet-repaint"
rm -rf "$out"
mkdir -p "$out"

export HOME
HOME="$(mktemp -d)"
unset XDG_CONFIG_HOME || true

cat > /tmp/lux-sheet-window-id.swift << 'SWIFT'
import CoreGraphics
import Foundation
let pid = Int(CommandLine.arguments.dropFirst().first ?? "") ?? -1
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
var best = 0
var bestArea = 0.0
for w in info {
    let owner = w[kCGWindowOwnerPID as String] as? Int ?? 0
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if owner != pid || layer != 0 { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width * height > bestArea {
        bestArea = width * height
        best = w[kCGWindowNumber as String] as? Int ?? 0
    }
}
if best == 0 { exit(1) }
print(best)
SWIFT

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT

sample="$DRIVE_WORK/sheet-sample.txt"
printf 'before the sheet\n' > "$sample"
drive_launch "$sample"

field_wait() {
  local name="$1" pattern="$2" _
  for _ in $(seq 1 40); do
    drive_dump
    if drive_field "$name" | grep -q -- "$pattern"; then
      return 0
    fi
    sleep 0.25
  done
  echo "$name never matched $pattern" >&2
  cat "$DRIVE_PROBE" >&2 || true
  return 1
}

# Sheets attached to any Lux window. osascript errors go to sheets.err so a
# missing Accessibility grant is not mistaken for "no sheet".
sheets() {
  local count
  if count="$(osascript -e "tell application \"System Events\" to tell (first process whose unix id is $DRIVE_PID)
    set n to 0
    repeat with w in windows
      set n to n + (count of sheets of w)
    end repeat
    return n
  end tell" 2>>"$out/sheets.err")"; then
    echo "$count"
  else
    echo 0
  fi
}

shoot_window() {
  local id
  id="$(swift /tmp/lux-sheet-window-id.swift "$DRIVE_PID")"
  screencapture -x -o -l "$id" "$1"
  [[ -s "$1" ]] || {
    echo "screencapture produced no file at $1" >&2
    exit 1
  }
}

field_wait active_path 'sheet-sample.txt'
field_wait text 'before the sheet'

# Command-O (key code 31).
drive_key tap 31 cmd
up=0
for _ in $(seq 1 40); do
  if [[ "$(sheets)" -ge 1 ]]; then
    up=1
    break
  fi
  sleep 0.25
done
if [[ "$up" != 1 ]]; then
  screencapture -x "$out/no-sheet.png" || true
  echo "Cmd+O did not attach a sheet to the Lux window" >&2
  cat "$out/sheets.err" >&2 || true
  tail -40 "$DRIVE_LOG" >&2 || true
  exit 1
fi
sleep 0.5
screencapture -x "$out/sheet-screen.png"
shoot_window "$out/editor-behind-sheet-1.png"

# The probe is answered from `about_to_wait` on the main thread. Reading it
# at all while the sheet is up means the main event loop is not parked.
drive_dump
first="$(drive_field redraws)"
sleep 2.5
drive_dump
second="$(drive_field redraws)"
echo "redraws while the sheet is up: $first -> $second"
if [[ -z "$first" || -z "$second" ]] || (( second < first + 3 )); then
  echo "the editor did not keep redrawing behind the sheet ($first -> $second)" >&2
  exit 1
fi

# An external edit reaches the buffer and the pixels behind the sheet.
printf 'changed behind the sheet\n' > "$sample"
field_wait text 'changed behind the sheet'
sleep 0.5
shoot_window "$out/editor-behind-sheet-2.png"
if [[ "$(sheets)" -lt 1 ]]; then
  echo "the sheet closed on its own" >&2
  exit 1
fi
if cmp -s "$out/editor-behind-sheet-1.png" "$out/editor-behind-sheet-2.png"; then
  echo "window pixels did not change behind the sheet" >&2
  exit 1
fi

# Escape cancels the panel; keyboard focus goes back to the editor.
drive_key tap 53 none
for _ in $(seq 1 40); do
  [[ "$(sheets)" -lt 1 ]] && break
  sleep 0.25
done
if [[ "$(sheets)" -ge 1 ]]; then
  echo "Escape did not close the sheet" >&2
  exit 1
fi
sleep 0.3
drive_key chars "Z"
field_wait text 'Z'
shoot_window "$out/after-sheet.png"

{
  echo "redraws_while_sheet_up=$first->$second"
  echo "text_after_escape=$(drive_field text)"
} > "$out/summary.txt"
cat "$out/summary.txt"
echo "editor repaints behind the Open sheet"
