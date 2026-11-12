#!/usr/bin/env bash
# Real Lux.app on macOS: double-click selects a word, then a right click
# opens the buffer menu over that selection.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/buffer-menu"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/buffer-menu.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/buffer-menu.rs"
printf 'alpha beta gamma\n' > "$fixture"

drive_init "$root/target/Lux.app"
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/word-selected.png" \
  --snapshot-on-usr1 "$out/context-menu.png" \
  --snapshot-on-usr1 "$out/context-menu-flipped.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == *buffer-menu.rs ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "buffer-menu.rs never became active" >&2
  cat "$DRIVE_PROBE" >&2 || true
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi

capture() {
  local dest="$1"
  kill -USR1 "$DRIVE_PID"
  local attempt
  for attempt in $(seq 1 40); do
    if [[ -s "$dest" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "snapshot missing: $dest" >&2
  tail -40 "$DRIVE_LOG" >&2 || true
  exit 1
}

window_bounds() {
  swift /tmp/lux-find-window.swift "$DRIVE_OWNER"
}

require_menu_geometry() {
  local mode="$1"
  python3 - "$(drive_field edit_menu_geom)" "$mode" << 'PY'
import sys
raw, mode = sys.argv[1], sys.argv[2]
if raw in ("", "0"):
    raise SystemExit("edit_menu_geom is empty")
parts = raw.split(";")
if len(parts) < 3:
    raise SystemExit(f"edit_menu_geom too short: {raw}")

def box(text):
    return [float(piece) for piece in text.split(",")]

window_w, window_h, mouse_x, mouse_y = box(parts[0])
panel = box(parts[1])
rows = [box(piece) for piece in parts[2:]]
px, py, pw, ph = panel

def inside(outer, inner, label):
    ox, oy, ow, oh = outer
    ix, iy, iw, ih = inner
    if ix + 0.6 < ox or iy + 0.6 < oy or ix + iw > ox + ow + 0.6 or iy + ih > oy + oh + 0.6:
        raise SystemExit(f"{label} {inner} is outside {outer}")

if px < -0.6 or py < -0.6 or px + pw > window_w + 0.6 or py + ph > window_h + 0.6:
    raise SystemExit(f"panel {panel} is outside the window {window_w}x{window_h}")
for index, row in enumerate(rows):
    inside(panel, row, f"row {index}")
fits_right = mouse_x + pw <= window_w + 0.6
fits_below = mouse_y + ph <= window_h + 0.6
if mode == "anchored":
    if fits_right and abs(px - mouse_x) > 1.5:
        raise SystemExit(f"menu x {px} is not the pointer {mouse_x}")
    if fits_below and abs(py - mouse_y) > 1.5:
        raise SystemExit(f"menu y {py} is not the pointer {mouse_y}")
elif mode == "flipped":
    if px + 1.0 >= mouse_x and py + 1.0 >= mouse_y:
        raise SystemExit(f"corner menu {panel} did not flip off the pointer {mouse_x},{mouse_y}")
else:
    raise SystemExit(f"unknown mode {mode}")
print(f"{mode} panel {panel} pointer {mouse_x:.0f},{mouse_y:.0f}")
PY
}

selected_word() {
  local sel start end text
  sel="$(drive_field sel)"
  start="${sel%%:*}"
  end="${sel##*:}"
  text="$(drive_field text)"
  if [[ -z "$start" || -z "$end" || "$start" == "$end" ]]; then
    return 1
  fi
  python3 - "$text" "$start" "$end" << 'PY'
import sys
text, start, end = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
word = text[start:end]
if word and " " not in word and "\n" not in word:
    print(word)
    raise SystemExit(0)
raise SystemExit(1)
PY
}

read -r origin_x origin_y win_w win_h <<<"$(window_bounds)"
picked=""
click_x=""
click_y=""
for dy in 70 88 106 124 142; do
  for dx in 90 130 170 210; do
    click_x=$((origin_x + dx))
    click_y=$((origin_y + dy))
    drive_key double "$click_x" "$click_y"
    sleep 0.25
    drive_dump || true
    if picked="$(selected_word)"; then
      break 2
    fi
  done
done
if [[ -z "$picked" ]]; then
  echo "double click did not select a word" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
echo "double click selected $picked at $click_x $click_y"
capture "$out/word-selected.png"

kept="$(drive_field sel)"
drive_key right "$click_x" "$click_y"
sleep 0.3
drive_dump
if [[ "$(drive_field edit_menu)" != 1* ]]; then
  echo "right click did not open the buffer menu" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
if [[ "$(drive_field sel)" != "$kept" ]]; then
  echo "right click inside the word changed the selection" >&2
  echo "before $kept" >&2
  echo "after $(drive_field sel)" >&2
  exit 1
fi
echo "menu $(drive_field edit_menu)"
require_menu_geometry anchored
capture "$out/context-menu.png"

read -r origin_x origin_y win_w win_h <<<"$(window_bounds)"
corner_x=$((origin_x + win_w - 36))
corner_y=$((origin_y + win_h - 40))
drive_key right "$corner_x" "$corner_y"
sleep 0.3
drive_dump
if [[ "$(drive_field edit_menu)" != 1* ]]; then
  echo "corner right click did not open the buffer menu" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
require_menu_geometry flipped
capture "$out/context-menu-flipped.png"
drive_stop
echo "buffer menu captures written"
