#!/usr/bin/env bash
# Real Lux.app on macOS. A file created beside the open folder appears in
# the sidebar, then disappears when it is removed.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/tree-watch"
mkdir -p "$out"
rm -f "$out"/*.png

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

fixture="$HOME/tree-watch"
mkdir -p "$fixture"
printf 'keep\n' > "$fixture/keep.txt"
arrived="$fixture/arrived.txt"

drive_init "$root/target/Lux.app"
drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/arrived.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field sidebar)" == 1 && "$(drive_field tree)" == *0:keep.txt* ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "the workspace tree never listed keep.txt" >&2
  cat "$DRIVE_PROBE" >&2 || true
  cat "$DRIVE_LOG" >&2 || true
  exit 1
fi

printf 'arrived\n' > "$arrived"
shown=0
for _ in $(seq 1 40); do
  drive_dump || true
  if [[ "$(drive_field tree)" == *0:arrived.txt* ]]; then
    shown=1
    break
  fi
  sleep 0.25
done
if [[ "$shown" != 1 ]]; then
  echo "arrived.txt did not appear in the sidebar: $(drive_field tree)" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi

rm -f "$out/arrived.png"
kill -USR1 "$DRIVE_PID"
captured=0
for _ in $(seq 1 40); do
  if [[ -s "$out/arrived.png" ]]; then
    captured=1
    break
  fi
  sleep 0.25
done
if [[ "$captured" != 1 ]]; then
  echo "snapshot missing: $out/arrived.png" >&2
  tail -40 "$DRIVE_LOG" >&2 || true
  exit 1
fi

rm -f "$arrived"
gone=0
for _ in $(seq 1 40); do
  drive_dump || true
  tree="$(drive_field tree)"
  if [[ "$tree" == *0:keep.txt* && "$tree" != *arrived.txt* ]]; then
    gone=1
    break
  fi
  sleep 0.25
done
if [[ "$gone" != 1 ]]; then
  echo "arrived.txt stayed in the sidebar: $(drive_field tree)" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi

drive_stop
echo "tree watch capture written"
