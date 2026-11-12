#!/usr/bin/env bash
# Real Lux.app on macOS: a triple click selects the whole line.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/triple-click"
mkdir -p "$out"
rm -f "$out"/*.png

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/triple-click.rs"
printf 'alpha beta gamma\ndelta epsilon\n' > "$fixture"

drive_init "$root/target/Lux.app"
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/line-selected.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == *triple-click.rs ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "triple-click.rs never became active" >&2
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

selected_line() {
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
raw, start, end = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
out, i = [], 0
while i < len(raw):
    if raw[i] == "\\" and i + 1 < len(raw):
        nxt = raw[i + 1]
        out.append({"n": "\n", "r": "\r", "\\": "\\"}.get(nxt, "\\" + nxt))
        i += 2
    else:
        out.append(raw[i])
        i += 1
text = "".join(out)
if start > end:
    start, end = end, start
chunk = text[start:end]
if chunk.startswith("alpha beta gamma") and "delta" not in chunk:
    print(chunk.replace("\n", "\\n"))
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
    drive_key triple "$click_x" "$click_y"
    sleep 0.3
    drive_dump || true
    if picked="$(selected_line)"; then
      break 2
    fi
  done
done
if [[ -z "$picked" ]]; then
  echo "triple click did not select the line" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
echo "triple click selected $picked at $click_x $click_y"
capture "$out/line-selected.png"
drive_stop
echo "triple click capture written"
