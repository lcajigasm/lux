#!/usr/bin/env bash
# macos-15: the unified title bar (ST-MAC-02). The compiled app's tab row
# is the window's title bar: it starts at the top of the window frame, the
# tabs start after the traffic lights, and native fullscreen (Control-
# Command-F) removes that inset and restores it on exit. Positions come
# from AppKit through the probe; the PNGs are screencaptures of the window.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "unified-title-bar runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

real_home="${HOME}"
out="$root/docs/evidence/unified-title-bar"
rm -rf "$out"
mkdir -p "$out"
report="$out/report.txt"
: > "$report"

die() {
  echo "FAIL $1" | tee -a "$report" >&2
  if [[ -n "${DRIVE_PROBE:-}" && -s "$DRIVE_PROBE" ]]; then
    grep -E '^(tab_row|title_bar|tab_strip|fullscreen)' "$DRIVE_PROBE" >&2 || true
  fi
  tail -40 "${DRIVE_LOG:-/dev/null}" >&2 || true
  exit 1
}
pass() { echo "PASS $1" | tee -a "$report"; }

cat > /tmp/lux-title-bar-window.swift << 'SWIFT'
import CoreGraphics
import Foundation
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 || owner.lowercased() != "lux" { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    print("\(w[kCGWindowNumber as String] as? Int ?? 0) \(Int(width))")
    exit(0)
}
exit(1)
SWIFT

# A fullscreen Space can still be animating in: retry until the window
# is on screen and capturable.
shoot() {
  local dest="$1" id _
  for _ in $(seq 1 20); do
    rm -f "$dest"
    local line
    if line="$(swift /tmp/lux-title-bar-window.swift)"; then
      id="${line%% *}"
      WINDOW_PT="${line##* }"
      screencapture -x -l "$id" -o "$dest" || true
      [[ -s "$dest" ]] && return 0
    fi
    sleep 0.5
  done
  die "screencapture produced no file at $dest"
}

home="$(mktemp -d)"
files="$(mktemp -d)"
printf 'first\n' > "$files/first.txt"
printf 'second\n' > "$files/second.txt"

echo "--> Building lux"
if [[ -z "${LUX_PREBUILT:-}" || ! -x "$root/target/debug/lux" ]]; then
  HOME="$real_home" cargo build -p lux --locked
else
  echo "--> prebuilt binary, compile skipped"
fi
app="$(HOME="$real_home" bash "$root/scripts/stage-lux-app.sh" --no-build)"
echo "--> bundled app $app" | tee -a "$report"

export HOME="$home"
drive_init "$app"
cleanup() {
  drive_stop || true
  rm -rf "$home" "$files"
}
trap cleanup EXIT

drive_launch "$files/first.txt" "$files/second.txt"
sleep 0.5

# Waits until the probe shows fullscreen=$1 with the inset rule for it.
wait_state() {
  local want="$1" _
  for _ in $(seq 1 40); do
    drive_dump || die "probe died"
    local fs inset
    fs="$(drive_field fullscreen)"
    inset="$(drive_field title_bar_inset_pt)"
    if [[ "$fs" == "$want" ]]; then
      if [[ "$want" == 1 && "$inset" == 0 ]]; then return 0; fi
      if [[ "$want" == 0 && "$inset" != 0 ]]; then return 0; fi
    fi
    sleep 0.5
  done
  die "fullscreen=$want not reached (fullscreen=$(drive_field fullscreen) inset=$(drive_field title_bar_inset_pt))"
}

check_windowed() {
  local label="$1"
  wait_state 0
  local row_y inset left
  row_y="$(drive_field tab_row_window_y_pt)"
  inset="$(drive_field title_bar_inset_pt)"
  left="$(drive_field tab_strip_left_pt)"
  echo "$label tab_row_window_y_pt=$row_y title_bar_inset_pt=$inset tab_strip_left_pt=$left" | tee -a "$report"
  awk -v y="$row_y" 'BEGIN { exit !(y >= 0 && y < 40) }' \
    || die "$label: tab row starts ${row_y}pt below the window top, want < 40"
  pass "$label: tab row at the top of the window (${row_y}pt)"
  awk -v i="$inset" 'BEGIN { exit !(i >= 60 && i <= 120) }' \
    || die "$label: traffic-light inset ${inset}pt is not a plausible button width"
  awk -v l="$left" -v i="$inset" 'BEGIN { exit !(l >= i) }' \
    || die "$label: first tab at ${left}pt overlaps the traffic lights (${inset}pt)"
  pass "$label: tabs start after the traffic lights (${left}pt >= ${inset}pt)"
}

# The traffic lights must stand out from the tab row behind them.
check_lights() {
  local png="$1" label="$2"
  python3 -I "$root/scripts/title_bar_pixels.py" "$png" "$WINDOW_PT" | tee -a "$report" \
    || die "$label: traffic lights not visible on the tab bar"
  pass "$label: traffic lights visible on the tab bar"
}

check_windowed "windowed"
shoot "$out/windowed.png"
check_lights "$out/windowed.png" "dark theme"

# Control-Command-F: View > Enter Full Screen (F is key code 3).
drive_focus
drive_key tap 3 cmd,ctrl
wait_state 1
pass "fullscreen: inset removed, first tab at $(drive_field tab_strip_left_pt)pt"
shoot "$out/fullscreen.png"

drive_focus
drive_key tap 3 cmd,ctrl
check_windowed "after fullscreen"
shoot "$out/restored.png"

# A light theme: the window takes the light appearance and the buttons
# still read on the light tab bar.
mkdir -p "$HOME/.config/lux"
printf '{ "workbench.colorTheme": "celeste" }\n' > "$HOME/.config/lux/settings.json"
drive_launch "$files/first.txt"
sleep 1
check_windowed "light theme"
shoot "$out/light.png"
check_lights "$out/light.png" "light theme"

echo "unified title bar scenario passed" | tee -a "$report"
