#!/usr/bin/env bash
# H1–H8 on one macos-15 runner, release Lux.app. A missed budget is data.
# The job fails only when a Lux measurement could not be taken.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
raw="$out/perf-raw.txt"
: > "$raw"
log() { echo "$*" | tee -a "$raw"; }

app="$(bash "$root/scripts/stage-lux-app.sh" --release)"
drive_init "$app"
trap drive_stop EXIT

median() {
  python3 - "$@" << 'PY'
import statistics, sys
print(f"{statistics.median([float(x) for x in sys.argv[1:]]):.1f}")
PY
}

first_frame_ms() {
  local line
  line="$(grep 'Lux: first frame' "$DRIVE_LOG" | tail -1 || true)"
  echo "${line}" | awk '{print $(NF-1)}'
}

buffer_frame_ms() {
  local _
  for _ in $(seq 1 120); do
    if grep -q 'Lux: buffer frame' "$DRIVE_LOG"; then
      grep 'Lux: buffer frame' "$DRIVE_LOG" | tail -1 | awk '{print $(NF-1)}'
      return 0
    fi
    if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
      return 1
    fi
    sleep 0.5
  done
  return 1
}

log "--> H1 cold start, 10 empty windows"
h1=()
for _ in $(seq 1 10); do
  empty="$DRIVE_WORK/empty-$_.txt"
  printf '' > "$empty"
  drive_launch "$empty"
  ms="$(first_frame_ms)"
  log "cold $_ ${ms} ms"
  if [[ -z "$ms" ]]; then
    echo "missing first frame" >&2
    cat "$DRIVE_LOG" >&2 || true
    exit 1
  fi
  h1+=("$ms")
  drive_stop
done
h1_median="$(median "${h1[@]}")"
log "H1 median $h1_median ms"

measure_open() {
  local bytes="$1" label="$2"
  local big="$DRIVE_WORK/$label.txt" small="$DRIVE_WORK/warm-$label.txt"
  python3 - "$big" "$bytes" << 'PY'
import sys
path, nbytes = sys.argv[1], int(sys.argv[2])
line = b"line\n"
with open(path, "wb") as f:
    written = 0
    while written + len(line) <= nbytes:
        f.write(line)
        written += len(line)
    if written < nbytes:
        f.write(b"x" * (nbytes - written))
PY
  printf 'warm\n' > "$small"
  drive_launch "$small" --bench-open "$big"
  ms0="$(first_frame_ms)"
  if [[ -z "$ms0" ]]; then
    echo "warm start produced no first frame" >&2
    exit 1
  fi
  sleep 0.4
  local idle
  idle="$(ps -o rss= -p "$DRIVE_PID" | tr -d ' ')"
  kill -USR2 "$DRIVE_PID"
  local ms
  if ! ms="$(buffer_frame_ms)"; then
    echo "no buffer frame for $label" >&2
    tail -40 "$DRIVE_LOG" >&2 || true
    exit 1
  fi
  sleep 0.3
  local hot extra ratio
  hot="$(ps -o rss= -p "$DRIVE_PID" | tr -d ' ')"
  extra=$((hot - idle))
  ratio="$(python3 - "$extra" "$bytes" << 'PY'
import sys
extra_kb, nbytes = int(sys.argv[1]), int(sys.argv[2])
print(f"{(extra_kb * 1024) / nbytes:.2f}")
PY
)"
  log "$label open ${ms} ms extra_rss_kb ${extra} ratio ${ratio}"
  drive_stop
  OPEN_MS="$ms"
  OPEN_RATIO="$ratio"
}

log "--> H3 open 10 MB"
measure_open $((10 * 1024 * 1024)) h3
h3_ms="$OPEN_MS"

log "--> H4 and H6 open 100 MB"
measure_open $((100 * 1024 * 1024)) h4
h4_ms="$OPEN_MS"
h6_ratio="$OPEN_RATIO"

log "--> H2 key to next presented frame, 20k lines"
big="$DRIVE_WORK/lines-20k.txt"
python3 - "$big" << 'PY'
import sys
with open(sys.argv[1], "w") as f:
    for i in range(20000):
        f.write(f"line {i}\n")
PY
drive_launch "$big"
sleep 0.5
drive_key chars "abcdefghijklmnopqrstuvwxyz0123456789ABCD"
sleep 0.6
drive_dump
h2_us="$(drive_field key_p99_us)"
h2_n="$(drive_field key_n)"
log "H2 p99 ${h2_us} us over ${h2_n} keys"
if [[ -z "$h2_n" || "$h2_n" -lt 10 ]]; then
  echo "H2 did not record enough key samples ($h2_n)" >&2
  exit 1
fi
drive_stop

log "--> H5 scroll frames, 50k lines"
scroll="$DRIVE_WORK/lines-50k.txt"
python3 - "$scroll" << 'PY'
import sys
with open(sys.argv[1], "w") as f:
    for i in range(50000):
        f.write(f"line {i} ....\n")
PY
drive_launch "$scroll"
sleep 0.5
for _ in $(seq 1 40); do
  drive_key scroll
  sleep 0.05
done
sleep 0.4
drive_dump
h5_us="$(drive_field frame_max_us)"
h5_n="$(drive_field frame_n)"
log "H5 max ${h5_us} us over ${h5_n} frames"
if [[ -z "$h5_n" || "$h5_n" -lt 5 ]]; then
  echo "H5 did not record enough frames ($h5_n)" >&2
  exit 1
fi
drive_stop

window_ms() {
  local owner="$1"
  local start now
  start="$(python3 -c 'import time; print(time.time())')"
  for _ in $(seq 1 40); do
    if swift /tmp/lux-find-window.swift "$owner" >/dev/null 2>&1; then
      now="$(python3 -c 'import time; print(time.time())')"
      python3 - "$start" "$now" << 'PY'
import sys
print(f"{(float(sys.argv[2]) - float(sys.argv[1])) * 1000:.1f}")
PY
      return 0
    fi
    sleep 0.1
  done
  return 1
}

subl_cold="not measured"
zed_cold="not measured"
export HOMEBREW_NO_AUTO_UPDATE=1
export NONINTERACTIVE=1
log "--> Sublime and Zed cold start (window listed, not first presented frame)"
if brew install --cask sublime-text >> "$raw" 2>&1; then
  subl="$(command -v subl || true)"
  if [[ -n "$subl" ]]; then
    subl_start="$(python3 -c 'import time; print(time.time())')"
    "$subl" -n "$DRIVE_WORK/empty-1.txt" >/dev/null 2>&1 || true
    if subl_seen="$(window_ms "Sublime Text")"; then
      subl_cold="$(python3 - "$subl_start" << 'PY'
import sys, time
print(f"{(time.time() - float(sys.argv[1])) * 1000:.1f} ms")
PY
)"
      log "Sublime window $subl_cold (poll segment $subl_seen ms)"
    else
      subl_cold="window did not appear"
      log "Sublime window did not appear"
    fi
    osascript -e 'tell application "Sublime Text" to quit' >/dev/null 2>&1 || true
  else
    subl_cold="subl not on PATH"
  fi
else
  subl_cold="brew install sublime-text failed"
fi
if brew install --cask zed >> "$raw" 2>&1; then
  zed_start="$(python3 -c 'import time; print(time.time())')"
  if open -a "Zed" "$DRIVE_WORK/empty-1.txt" >> "$raw" 2>&1; then
    if zed_seen="$(window_ms Zed)"; then
      zed_cold="$(python3 - "$zed_start" << 'PY'
import sys, time
print(f"{(time.time() - float(sys.argv[1])) * 1000:.1f} ms")
PY
)"
      log "Zed window $zed_cold (poll segment $zed_seen ms)"
    else
      zed_cold="window did not appear"
      log "Zed window did not appear"
    fi
  else
    zed_cold="open -a Zed failed"
  fi
  osascript -e 'tell application "Zed" to quit' >/dev/null 2>&1 || true
else
  zed_cold="brew install zed failed"
fi

h2_ms="$(python3 - "$h2_us" << 'PY'
import sys
print(f"{int(sys.argv[1]) / 1000:.2f}")
PY
)"
h5_ms="$(python3 - "$h5_us" << 'PY'
import sys
print(f"{int(sys.argv[1]) / 1000:.2f}")
PY
)"

cat > "$out/perf.md" << EOF
# macos-15 release measurements

Lux columns for H1 and H3/H4 are the process's own clock: start until the first presented frame, and SIGUSR2 open until the first presented frame of that buffer. Sublime and Zed cold start is process launch until a window is listed. That is not the same clock, so those cells are not a Lux comparison. Key-to-pixel and scroll-frame times are inside the Lux process. They are not readable from Sublime or Zed.

| Id | Measurement | Budget | Lux | Sublime | Zed |
|---|---|---|---|---|---|
| H1 | Cold start, median of 10 | < 150 ms | ${h1_median} ms | ${subl_cold} | ${zed_cold} |
| H2 | Key to next presented frame, p99, 20k lines | < 8 ms | ${h2_ms} ms (n=${h2_n}) | not measurable the same way | not measurable the same way |
| H3 | Open 10 MB, warm process | < 150 ms | ${h3_ms} ms | not measurable the same way | not measurable the same way |
| H4 | Open 100 MB, warm process | < 1 s | ${h4_ms} ms | not measurable the same way | not measurable the same way |
| H5 | Scroll 50k lines, slowest recorded frame | no frame > 16 ms | ${h5_ms} ms (n=${h5_n}) | not measurable the same way | not measurable the same way |
| H6 | Extra RSS while opening 100 MB | < 3× file | ${h6_ratio}× | not measurable the same way | not measurable the same way |
| H7 | Same method on all three | — | H1's Lux cell is first presented frame. Sublime and Zed are window-listed. H2 and H5 have no external equivalent. | window-listed only | window-listed only |

Budgets are not a job failure. The numbers above are the run.
EOF
log "--> wrote $out/perf.md"
cat "$out/perf.md"
