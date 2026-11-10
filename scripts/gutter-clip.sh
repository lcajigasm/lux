#!/usr/bin/env bash
# Real Lux.app on macOS: a 400-column by 400-line file, wheel scrolls,
# and pixel checks that the line-number margin never takes buffer ink.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/gutter"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/gutter-clip.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/wide-lines.txt"
python3 - "$fixture" << 'PY'
import sys
path = sys.argv[1]
line = "M" * 420
with open(path, "w", encoding="utf-8") as handle:
    for _ in range(400):
        handle.write(line + "\n")
PY

drive_init "$root/target/Lux.app"
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/01-before.png" \
  --snapshot-on-usr1 "$out/02-after-shift-wheel.png" \
  --snapshot-on-usr1 "$out/03-after-delta-x.png" \
  --snapshot-on-usr1 "$out/04-after-vertical.png" \
  --snapshot-on-usr1 "$out/05-wrap-on.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == *wide-lines.txt ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "the wide file never became active" >&2
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi
if [[ "$(drive_field language)" != "Plain Text" ]]; then
  echo "wide-lines.txt language is $(drive_field language), want Plain Text" >&2
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

wheel_at() {
  local kind="$1"
  local lines="$2"
  local line x y w h cx cy
  line="$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
  read -r x y w h <<<"$line"
  cx=$((x + w / 2))
  cy=$((y + h / 2))
  drive_key "$kind" "$cx" "$cy" "$lines"
}

require_scroll() {
  local field="$1"
  local before="$2"
  local label="$3"
  drive_dump
  local after
  after="$(drive_field "$field")"
  if [[ "$after" == "$before" ]]; then
    echo "$label left $field at $before" >&2
    exit 1
  fi
  echo "$label $field $before -> $after"
}

capture "$out/01-before.png"

drive_dump
x0="$(drive_field scroll_x)"
y0="$(drive_field scroll_y)"
wheel_at scrollshift -8
sleep 0.4
require_scroll scroll_x "$x0" "shift+wheel"
capture "$out/02-after-shift-wheel.png"

drive_dump
x1="$(drive_field scroll_x)"
wheel_at scrollx -8
sleep 0.4
require_scroll scroll_x "$x1" "delta-x"
capture "$out/03-after-delta-x.png"

drive_dump
y1="$(drive_field scroll_y)"
wheel_at scroll -8
sleep 0.4
require_scroll scroll_y "$y1" "vertical"
drive_dump
y2="$(drive_field scroll_y)"
capture "$out/04-after-vertical.png"

# Option+Z. macOS virtual key for Z is 6.
drive_key tap 6 alt
sleep 0.4
drive_dump
if [[ "$(drive_field word_wrap)" != "1" ]]; then
  echo "Option+Z did not enable word wrap" >&2
  exit 1
fi
wheel_at scrollshift -8
sleep 0.2
wheel_at scrollx -8
sleep 0.4
drive_dump
if [[ "$(drive_field scroll_x)" != "0" ]]; then
  echo "wrap still has scroll_x=$(drive_field scroll_x)" >&2
  exit 1
fi
if [[ "$(drive_field word_wrap)" != "1" ]]; then
  echo "word wrap did not stay on" >&2
  exit 1
fi
capture "$out/05-wrap-on.png"

python3 "$root/scripts/gutter_pixels.py" "$out"

{
  echo "gutter clip captures"
  echo "shift+wheel scroll_x $x0 -> $x1"
  echo "vertical scroll_y $y1 -> $y2"
  echo "wrap scroll_x 0"
  ls -1 "$out"/*.png
} | tee "$out/gutter-clip.txt"

drive_stop
