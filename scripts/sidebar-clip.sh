#!/usr/bin/env bash
# Real Lux.app on macOS. A folder with more than 80 rows is scrolled to
# the middle and to the end. The header band and the status-bar band in
# the sidebar column must stay pixel-identical. The rows between them must
# move. A row that paints over the title or under the status bar fails.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/sidebar-clip"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/sidebar-clip.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

folder="$HOME/clip-project"
mkdir -p "$folder/.claude"
printf 'rules\n' > "$folder/.claude/rules.txt"
python3 - "$folder" << 'PY'
import sys
from pathlib import Path
root = Path(sys.argv[1])
for index in range(90):
    (root / f"entry-{index:02d}.txt").write_text(f"row {index}\n", encoding="utf-8")
(root / "zz-last.txt").write_text("last row\n", encoding="utf-8")
count = sum(1 for _ in root.iterdir())
if count < 80:
    raise SystemExit(f"fixture has {count} entries, want more than 80")
PY

drive_init "$root/target/Lux.app"
drive_launch "$folder" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/01-top.png" \
  --snapshot-on-usr1 "$out/02-middle.png" \
  --snapshot-on-usr1 "$out/03-end.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field sidebar)" == "1" ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "the folder never opened a sidebar" >&2
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

sidebar_wheel() {
  local lines="$1"
  local line x y w h
  line="$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
  read -r x y w h <<<"$line"
  # The sidebar is the left strip. A wheel at the window center scrolls
  # the editor and leaves the tree where it is.
  drive_key scroll "$((x + 40))" "$((y + h / 2))" "$lines"
}

scroll_value() {
  drive_dump
  drive_field sidebar_scroll
}

scroll_until() {
  local target="$1"
  local direction="$2"
  local guard_n=0
  local now
  now="$(scroll_value)"
  while [[ "$guard_n" -lt 80 ]]; do
    if [[ "$direction" == "down" && "$now" -ge "$target" ]]; then
      echo "$now"
      return 0
    fi
    if [[ "$direction" == "up" && "$now" -le "$target" ]]; then
      echo "$now"
      return 0
    fi
    local before="$now"
    if [[ "$direction" == "down" ]]; then
      sidebar_wheel -6
    else
      sidebar_wheel 6
    fi
    sleep 0.15
    now="$(scroll_value)"
    if [[ "$now" == "$before" ]]; then
      echo "$now"
      return 0
    fi
    guard_n=$((guard_n + 1))
  done
  echo "sidebar scroll did not settle" >&2
  exit 1
}

capture "$out/01-top.png"
top="$(scroll_value)"
if [[ "$top" != "0" ]]; then
  echo "sidebar started at $top, want 0" >&2
  exit 1
fi

end="$(scroll_until 999999 down)"
if [[ "$end" -le 200 ]]; then
  echo "scrolling 90 rows only reached $end" >&2
  exit 1
fi
mid_target=$((end / 2))
middle="$(scroll_until "$mid_target" up)"
if [[ "$middle" -le 80 || "$middle" -ge "$end" ]]; then
  echo "middle scroll is $middle, end is $end" >&2
  exit 1
fi
capture "$out/02-middle.png"
finished="$(scroll_until 999999 down)"
if [[ "$finished" -le "$middle" ]]; then
  echo "end scroll $finished did not pass the middle $middle" >&2
  exit 1
fi
capture "$out/03-end.png"

python3 "$root/scripts/sidebar_clip_pixels.py" "$out"

{
  echo "sidebar clip captures"
  echo "entries include .claude and 90 numbered files"
  echo "scroll top $top middle $middle end $finished"
  ls -1 "$out"/*.png
} | tee "$out/sidebar-clip.txt"

drive_stop
