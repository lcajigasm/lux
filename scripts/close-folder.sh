#!/usr/bin/env bash
# Real Lux.app on macOS. Two folders open in the sidebar. The × on the
# second folder removes it from the sidebar and leaves the directory.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/close-folder"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/close-folder.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

alpha="$HOME/alpha-project"
beta="$HOME/beta-project"
mkdir -p "$alpha" "$beta"
printf 'keep\n' > "$alpha/keep.txt"
printf 'gone\n' > "$beta/gone.txt"

drive_init "$root/target/Lux.app"
# drive_launch appends its first argument after the extras, so alpha is
# the first folder and beta is the second.
drive_launch "$beta" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/closed.png" \
  "$alpha"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field roots)" == *alpha-project*beta-project* ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "both folders were not in the sidebar: $(drive_field roots)" >&2
  cat "$DRIVE_PROBE" >&2 || true
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi
if [[ "$(drive_field tree)" != *keep.txt* || "$(drive_field tree)" != *gone.txt* ]]; then
  echo "sidebar rows were $(drive_field tree)" >&2
  exit 1
fi

line="$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
read -r x y w h <<<"$line"
# Points: tab 32, header 28, one file row, then the second folder's ×.
click_x=$((x + 206))
click_y=$((y + 93))
drive_click "$click_x" "$click_y"
sleep 0.4

drive_dump
roots="$(drive_field roots)"
tree="$(drive_field tree)"
if [[ "$roots" != "alpha-project" ]]; then
  echo "close left roots=$roots" >&2
  echo "tree=$tree" >&2
  exit 1
fi
if [[ "$tree" != *keep.txt* || "$tree" == *gone.txt* || "$tree" == *beta-project* ]]; then
  echo "sidebar still shows the closed folder: $tree" >&2
  exit 1
fi
if [[ ! -f "$beta/gone.txt" || ! -d "$beta" ]]; then
  echo "closing the folder removed it from disk" >&2
  exit 1
fi
if [[ ! -f "$alpha/keep.txt" ]]; then
  echo "the folder that stayed lost its file" >&2
  exit 1
fi

kill -USR1 "$DRIVE_PID"
for _ in $(seq 1 40); do
  if [[ -s "$out/closed.png" ]]; then
    break
  fi
  sleep 0.25
done
if [[ ! -s "$out/closed.png" ]]; then
  echo "final snapshot missing" >&2
  exit 1
fi

{
  echo "close folder"
  echo "roots after close: $roots"
  echo "tree after close: $tree"
  echo "beta directory still exists"
  ls -1 "$out"/*.png
} | tee "$out/close-folder.txt"

drive_stop
