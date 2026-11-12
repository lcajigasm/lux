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
  --snapshot-on-usr1 "$out/context-menu.png"

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

window_origin() {
  local line x y w h
  line="$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
  read -r x y w h <<<"$line"
  echo "$x $y"
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

read -r origin_x origin_y <<<"$(window_origin)"
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
capture "$out/context-menu.png"
drive_stop
echo "buffer menu captures written"
