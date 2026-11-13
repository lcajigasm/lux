#!/usr/bin/env bash
# Real Lux.app on macOS: drag a selection to move it, Option-drag to copy it.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/selection-drag"
mkdir -p "$out"
rm -f "$out"/*.png

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/selection-drag.txt"
printf 'alpha beta gamma\n' > "$fixture"

drive_init "$root/target/Lux.app"
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/moved.png" \
  --snapshot-on-usr1 "$out/copied.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == *selection-drag.txt ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "selection-drag.txt never became active" >&2
  cat "$DRIVE_PROBE" >&2 || true
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi

capture() {
  local dest="$1"
  rm -f "$dest"
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
  echo "$x $y $w"
}

decode_text() {
  python3 - "$1" << 'PY'
import sys
raw = sys.argv[1]
out, i = [], 0
while i < len(raw):
    if raw[i] == "\\" and i + 1 < len(raw):
        nxt = raw[i + 1]
        out.append({"n": "\n", "r": "\r", "\\": "\\"}.get(nxt, "\\" + nxt))
        i += 2
    else:
        out.append(raw[i])
        i += 1
sys.stdout.write("".join(out))
PY
}

selected_word() {
  local sel start end text
  sel="$(drive_field sel)"
  start="${sel%%:*}"
  end="${sel##*:}"
  text="$(decode_text "$(drive_field text)")"
  python3 - "$text" "$start" "$end" << 'PY'
import sys
text, start, end = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
if start > end:
    start, end = end, start
chunk = text[start:end]
if chunk == "beta":
    print(f"{start} {end}")
    raise SystemExit(0)
raise SystemExit(1)
PY
}

read -r origin_x origin_y origin_w <<<"$(window_origin)"
picked=""
click_x=""
click_y=""
for dy in 70 88 106 124 142; do
  for dx in 70 100 130 160 190 220 260; do
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
  echo "double click did not select beta" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
read -r beta_start beta_end <<<"$picked"
echo "selected beta at $beta_start:$beta_end via $click_x $click_y"
# The next press has to be a fresh click. Inside 400ms it would count as a
# triple click and select the line instead of dragging the word.
sleep 0.5

drop_x=$((origin_x + origin_w - 40))
drive_key drag "$click_x" "$click_y" "$drop_x" "$click_y"
sleep 0.4
drive_dump

python3 - "$(drive_field text)" "$(drive_field sel)" "$beta_start" "$beta_end" << 'PY'
import sys
raw, sel, old_start, old_end = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
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
start_s, end_s = sel.split(":", 1)
start, end = int(start_s), int(end_s)
if start > end:
    start, end = end, start
chunk = text[start:end]
if chunk != "beta":
    raise SystemExit(f"moved selection is {chunk!r}, not beta; text={text!r}")
if text.count("beta") != 1:
    raise SystemExit(f"move left {text.count('beta')} copies: {text!r}")
if start == old_start:
    raise SystemExit(f"beta did not move: {text!r}")
print(f"moved beta to {start}:{end} text={text!r}")
PY
capture "$out/moved.png"

drive_key tap 6 cmd
sleep 0.4
drive_dump
restored="$(decode_text "$(drive_field text)")"
if [[ "$restored" != $'alpha beta gamma\n' && "$restored" != "alpha beta gamma" ]]; then
  echo "undo did not restore the line: ${restored@Q}" >&2
  exit 1
fi

drive_key double "$click_x" "$click_y"
sleep 0.25
drive_dump
picked="$(selected_word)"
read -r beta_start beta_end <<<"$picked"
sleep 0.5

drive_key drag "$click_x" "$click_y" "$drop_x" "$click_y" alt
sleep 0.4
drive_dump

python3 - "$(drive_field text)" "$(drive_field sel)" "$beta_start" "$beta_end" << 'PY'
import sys
raw, sel, old_start, old_end = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
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
start_s, end_s = sel.split(":", 1)
start, end = int(start_s), int(end_s)
if start > end:
    start, end = end, start
if text[start:end] != "beta":
    raise SystemExit(f"copied selection is {text[start:end]!r}; text={text!r}")
if text.count("beta") != 2:
    raise SystemExit(f"copy left {text.count('beta')} betas: {text!r}")
if text[old_start:old_end] != "beta":
    raise SystemExit(f"original beta was not kept: {text!r}")
print(f"copied beta to {start}:{end} text={text!r}")
PY

capture "$out/copied.png"
drive_stop
echo "selection drag capture written"
