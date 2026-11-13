#!/usr/bin/env bash
# Median of 10 release launches: process start until the first presented
# frame. A missing sample or a missing frame fails the job. A budget miss
# is written down and does not.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/h1"
rm -rf "$out"
mkdir -p "$out"

bash "$root/scripts/stage-lux-app.sh" --release
drive_init "$root/target/Lux.app"
empty="$(mktemp)"
: > "$empty"

drive_snap() {
  local path="$1"
  local _
  kill -USR1 "$DRIVE_PID"
  for _ in $(seq 1 40); do
    if [[ -s "$path" ]]; then
      return 0
    fi
    if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
      echo "Lux.app exited before $path" >&2
      return 1
    fi
    sleep 0.25
  done
  echo "timed out waiting for $path" >&2
  tail -30 "$DRIVE_LOG" >&2 || true
  return 1
}

for i in $(seq 1 10); do
  if [[ "$i" -eq 10 ]]; then
    drive_launch "$empty" --snapshot-scale 2 --snapshot-on-usr1 "$out/cold.png"
  else
    drive_launch "$empty"
  fi
  ms=""
  for _ in $(seq 1 40); do
    ms="$(awk '/^Lux: first frame [0-9.]+ ms$/ { print $4; exit }' "$DRIVE_LOG")"
    if [[ -n "$ms" ]]; then
      break
    fi
    sleep 0.25
  done
  if [[ -z "$ms" ]]; then
    echo "run $i has no first presented frame" >&2
    cp "$DRIVE_LOG" "$out/run-$i.log" || true
    exit 1
  fi
  printf '%s\n' "$ms" >> "$out/samples.txt"
  echo "run $i ${ms} ms"
  if [[ "$i" -eq 10 ]]; then
    drive_snap "$out/cold.png"
  fi
  cp "$DRIVE_LOG" "$out/run-$i.log"
  drive_stop || true
done

count="$(wc -l < "$out/samples.txt" | tr -d ' ')"
median="$(sort -n "$out/samples.txt" | awk 'NR==5 { print; exit }')"
if [[ "$count" -ne 10 || -z "$median" ]]; then
  echo "expected 10 samples, got ${count:-0}" >&2
  exit 1
fi

python3 "$root/scripts/frame_png.py" frame "$out/cold.png"

{
  echo "# H1"
  echo
  echo "Median of 10 release launches, process start to the first presented frame."
  echo "The median is the 5th of the 10 sorted samples, the same rule as \`scripts/p0-perf.sh\`."
  echo
  echo "Median: ${median} ms."
  echo
  echo "Budget: < 150 ms. A miss is recorded and does not fail the job."
  echo
  echo "Samples (ms):"
  echo
  cat "$out/samples.txt"
} > "$out/h1.md"

awk -v m="$median" 'BEGIN {
  if (m + 0 >= 150) print "H1 budget miss: " m " ms"
  else print "H1 within budget: " m " ms"
}'
echo "H1 median ${median} ms"
