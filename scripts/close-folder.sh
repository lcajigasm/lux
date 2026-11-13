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
echo "window $x $y $w $h"

capture() {
  rm -f "$out/closed.png"
  kill -USR1 "$DRIVE_PID"
  local attempt
  for attempt in $(seq 1 40); do
    if [[ -s "$out/closed.png" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "final snapshot missing" >&2
  return 1
}

# Open beta's file first, from the left of a row, so the close strip
# is not the thing we hit. A clean tab of that folder must leave with
# the folder. A dirty tab would stay; this file has no edits.
opened_file=0
for dy in $(seq 80 8 220); do
  for dx in 40 80 120; do
    drive_click "$((x + dx))" "$((y + dy))"
    sleep 0.2
    drive_dump || true
    case "$(drive_field active_path)" in
      *beta-project/gone.txt*) opened_file=1; break 2 ;;
    esac
    roots="$(drive_field roots)"
    if [[ "$roots" != "alpha-project|beta-project" ]]; then
      capture || true
      echo "opening gone.txt closed a folder: $roots" >&2
      exit 1
    fi
  done
done
if [[ "$opened_file" != 1 ]]; then
  capture || true
  echo "gone.txt did not open: $(drive_field active_path)" >&2
  exit 1
fi

# The window-list rect includes the title bar, so a point measured from
# the content origin (tab 32 + header 28 + one row) misses the ×. Walk
# the close strip from the lower rows upward and stop when beta leaves.
closed=0
click_x=""
click_y=""
roots=""
tree=""
for dy in $(seq 170 -8 56); do
  for dx in 196 206 216; do
    click_x=$((x + dx))
    click_y=$((y + dy))
    drive_click "$click_x" "$click_y"
    sleep 0.25
    drive_dump || true
    roots="$(drive_field roots)"
    tree="$(drive_field tree)"
    if [[ "$roots" == "alpha-project" && "$tree" == *keep.txt* && "$tree" != *gone.txt* && "$tree" != *beta-project* ]]; then
      closed=1
      echo "closed beta at $click_x $click_y"
      break 2
    fi
    if [[ "$roots" != "alpha-project|beta-project" ]]; then
      capture || true
      echo "click $click_x $click_y left roots=$roots" >&2
      echo "tree=$tree" >&2
      exit 1
    fi
  done
done
if [[ "$closed" != 1 ]]; then
  capture || true
  echo "close left roots=$roots" >&2
  echo "tree=$tree" >&2
  echo "window $x $y $w $h" >&2
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
drive_dump || true
tab_paths="$(drive_field tab_paths)"
active="$(drive_field active_path)"
if [[ "$tab_paths" == *beta-project* || "$active" == *beta-project* || "$tab_paths" == *gone.txt* ]]; then
  capture || true
  echo "a beta tab stayed open: tab_paths=$tab_paths active=$active" >&2
  exit 1
fi

capture

{
  echo "close folder"
  echo "roots after close: $roots"
  echo "tree after close: $tree"
  echo "tabs after close: $tab_paths"
  echo "active after close: $active"
  echo "beta directory still exists"
  echo "unsaved tabs stay open; this file was clean and its tab closed"
  ls -1 "$out"/*.png
} | tee "$out/close-folder.txt"

drive_stop
