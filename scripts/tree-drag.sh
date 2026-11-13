#!/usr/bin/env bash
# Real Lux.app on macOS: drag a sidebar file onto a folder.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/tree-drag"
mkdir -p "$out"
rm -f "$out"/*.png

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/tree-drag"
mkdir -p "$fixture/inbox"
printf 'tree drag\n' > "$fixture/move-me.txt"
moved="$fixture/inbox/move-me.txt"

drive_init "$root/target/Lux.app"
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/moved.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field sidebar)" == 1 && "$(drive_field tree)" == *0:move-me.txt* ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "the workspace tree never listed move-me.txt" >&2
  cat "$DRIVE_PROBE" >&2 || true
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi

read -r origin_x origin_y _origin_w _origin_h <<<"$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
# The file row is below the folder row. A miss stays a no-op: the folder
# dropped on the file lands back on the workspace root.
sx=$((origin_x + 80))
found=0
for sy in $(seq 64 18 240); do
  for dy in $(seq 48 18 $((sy - 12))); do
    drive_key drag "$sx" "$sy" "$sx" "$dy"
    sleep 0.15
    if [[ -f "$moved" && ! -f "$fixture/move-me.txt" ]]; then
      found=1
      echo "moved via $sx,$sy -> $sx,$dy"
      break 2
    fi
  done
done
if [[ "$found" != 1 ]]; then
  echo "drag did not move move-me.txt into inbox" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi

drive_dump
tree="$(drive_field tree)"
if [[ "$tree" != *0:inbox* || "$tree" != *1:move-me.txt* ]]; then
  echo "sidebar did not show move-me.txt inside inbox: $tree" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi
if [[ "$(cat "$moved")" != $'tree drag\n' && "$(cat "$moved")" != "tree drag" ]]; then
  echo "moved file contents changed: $(cat "$moved")" >&2
  exit 1
fi

rm -f "$out/moved.png"
kill -USR1 "$DRIVE_PID"
for _ in $(seq 1 40); do
  if [[ -s "$out/moved.png" ]]; then
    drive_stop
    echo "tree drag capture written"
    exit 0
  fi
  sleep 0.25
done
echo "snapshot missing: $out/moved.png" >&2
tail -40 "$DRIVE_LOG" >&2 || true
exit 1
