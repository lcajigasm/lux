#!/usr/bin/env bash
# Real Lux.app on macOS. Triple-click selects the line and a double click
# selects a word, both on Monokai. The pixel check records the painted
# selection and fails under SELECTION_CONTRAST_MIN.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
python3 "$root/scripts/selection_contrast.py" --self-test
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
mkdir -p "$HOME/.config/lux"
printf '%s\n' '{ "editor.fontSize": 13.0, "editor.lineHeight": 1.4, "workbench.colorTheme": "monokai" }' \
  > "$HOME/.config/lux/settings.json"
unset XDG_CONFIG_HOME || true

out="$root/docs/evidence/selection-contrast"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/colors.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/selection-contrast.rs"
printf 'alpha beta gamma\ndelta epsilon\n' > "$fixture"

drive_init "$root/target/Lux.app"
trap drive_stop EXIT
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/line-selected.png" \
  --snapshot-on-usr1 "$out/word-selected.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == *selection-contrast.rs ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "selection-contrast.rs never became active" >&2
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
  echo "$x $y"
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

selection_text() {
  local sel start end text
  sel="$(drive_field sel)"
  start="${sel%%:*}"
  end="${sel##*:}"
  text="$(decode_text "$(drive_field text)")"
  if [[ -z "$start" || -z "$end" || "$start" == "$end" ]]; then
    return 1
  fi
  python3 - "$text" "$start" "$end" "$1" << 'PY'
import sys
text, start, end, kind = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
if start > end:
    start, end = end, start
chunk = text[start:end]
if kind == "line":
    ok = chunk.startswith("alpha beta gamma") and "delta" not in chunk
else:
    ok = "\n" not in chunk and " " not in chunk and len(chunk) >= 3
if not ok:
    raise SystemExit(1)
sys.stdout.write(chunk.replace("\n", "\\n"))
PY
}

read -r origin_x origin_y <<<"$(window_origin)"
line_at=""
click_x=""
click_y=""
for dy in 70 88 106 124 142; do
  for dx in 90 130 170 210; do
    click_x=$((origin_x + dx))
    click_y=$((origin_y + dy))
    drive_key triple "$click_x" "$click_y"
    sleep 0.3
    drive_dump || true
    if line_at="$(selection_text line)"; then
      break 2
    fi
  done
done
if [[ -z "$line_at" ]]; then
  echo "triple click did not select the line" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
echo "triple click selected $line_at at $click_x $click_y"
capture "$out/line-selected.png"

word_at=""
word_x=""
word_y="$click_y"
for dx in 0 24 48 72 96 120 -24 -48; do
  word_x=$((click_x + dx))
  drive_key double "$word_x" "$word_y"
  sleep 0.3
  drive_dump || true
  if word_at="$(selection_text word)"; then
    break
  fi
  word_at=""
done
if [[ -z "$word_at" ]]; then
  echo "double click did not select a word" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
echo "double click selected $word_at at $word_x $word_y"
capture "$out/word-selected.png"
python3 "$root/scripts/selection_contrast.py" \
  "$out/line-selected.png" \
  "$out/word-selected.png" \
  "$out"
echo "selection contrast capture written"
