#!/usr/bin/env bash
# Median of 10 release launches. Before is the cold-start head
# (fonts already deferred). After also overlaps the GPU adapter
# with window creation.
# The median is the 5th of the 10 sorted samples. A missing sample or a
# missing frame fails the job. A budget miss is written down and does not.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/h1-overlap"
rm -rf "$out"
mkdir -p "$out"

before_sha="32aa8c5931a398e5aa7136052f30def5cf1d0075"

bash "$root/scripts/stage-lux-app.sh" --release
cp -R "$root/target/Lux.app" /tmp/Lux-after.app

git fetch --depth 1 origin "$before_sha"
git worktree add --detach /tmp/lux-before "$before_sha"
(
  cd /tmp/lux-before
  CARGO_TARGET_DIR="$root/target" cargo build -p lux --release --locked
)
mkdir -p /tmp/lux-before/target/release
cp "$root/target/release/lux" /tmp/lux-before/target/release/lux
(cd /tmp/lux-before && bash scripts/stage-lux-app.sh --release --no-build)
cp -R /tmp/lux-before/target/Lux.app /tmp/Lux-before.app

drive_snap() {
  local path="$1"
  local _
  kill -USR1 "$DRIVE_PID"
  for _ in $(seq 1 80); do
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

measure() {
  local label="$1"
  local samples="$out/samples-${label}.txt"
  : > "$samples"
  local i ms empty
  empty="$(mktemp)"
  : > "$empty"
  for i in $(seq 1 10); do
    if [[ "$label" == "after" && "$i" -eq 10 ]]; then
      drive_launch "$empty" --snapshot-scale 2 --snapshot-on-usr1 "$out/first.png"
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
      echo "$label run $i has no first presented frame" >&2
      cp "$DRIVE_LOG" "$out/${label}-${i}.log" || true
      exit 1
    fi
    printf '%s\n' "$ms" >> "$samples"
    echo "$label run $i ${ms} ms"
    if [[ "$label" == "after" && "$i" -eq 10 ]]; then
      drive_snap "$out/first.png"
      cp "$DRIVE_LOG" "$out/after-10.log"
    fi
    drive_stop || true
  done
}

drive_init /tmp/Lux-before.app
measure before
drive_init /tmp/Lux-after.app
measure after

median_of() {
  local count median
  count="$(wc -l < "$1" | tr -d ' ')"
  median="$(sort -n "$1" | awk 'NR==5 { print; exit }')"
  if [[ "$count" -ne 10 || -z "$median" ]]; then
    echo "expected 10 samples in $1, got ${count:-0}" >&2
    exit 1
  fi
  printf '%s\n' "$median"
}

before_ms="$(median_of "$out/samples-before.txt")"
after_ms="$(median_of "$out/samples-after.txt")"

python3 "$root/scripts/frame_png.py" frame "$out/first.png"

python3 - "$out/after-10.log" "$out/cause.txt" << 'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
found = {}
for name, ms in re.findall(r"Lux: phase (\w+) ([0-9.]+) ms", text):
    found[name] = float(ms)
frame = re.search(r"Lux: first frame ([0-9.]+) ms", text)
order = ["font", "app", "cli", "window", "adapter", "gpu"]
points = [(name, found[name]) for name in order if name in found]
if frame:
    points.append(("frame", float(frame.group(1))))
prev = 0.0
cause = "frame"
cause_ms = 0.0
lines = []
for name, ms in points:
    delta = ms - prev
    lines.append(f"{name} {ms:.1f} ms (delta {delta:.1f})")
    if delta > cause_ms:
        cause = name
        cause_ms = delta
    prev = ms
coverage = found.get("coverage")
body = "\n".join(lines)
if coverage is not None:
    body += f"\ncoverage {coverage:.1f} ms after the first frame"
body += f"\ncause {cause} {cause_ms:.1f} ms\n"
Path(sys.argv[2]).write_text(body, encoding="utf-8")
PY

cause="$(awk '/^cause / { print $2 " " $3; exit }' "$out/cause.txt")"

export HOMEBREW_NO_AUTO_UPDATE=1
export NONINTERACTIVE=1
subl_note="pending"
zed_note="pending"
printf 'warm\n' > "$DRIVE_WORK/compare.txt"
echo "--> Sublime and Zed, official cask" | tee -a "$out/raw.txt"
if brew install --cask sublime-text >> "$out/raw.txt" 2>&1; then
  subl="$(command -v subl || true)"
  if [[ -n "$subl" ]]; then
    "$subl" -n "$DRIVE_WORK/compare.txt" >/dev/null 2>&1 || true
    if swift /tmp/lux-find-window.swift "Sublime Text" >/dev/null 2>&1; then
      subl_note="window listed"
    else
      subl_note="pending: window did not appear"
    fi
    osascript -e 'tell application "Sublime Text" to quit' >/dev/null 2>&1 || true
  else
    subl_note="pending: subl not on PATH after the cask install"
  fi
else
  subl_note="pending: brew install --cask sublime-text failed"
fi
if brew install --cask zed >> "$out/raw.txt" 2>&1; then
  if open -a "Zed" "$DRIVE_WORK/compare.txt" >> "$out/raw.txt" 2>&1; then
    if swift /tmp/lux-find-window.swift Zed >/dev/null 2>&1; then
      zed_note="window listed"
    else
      zed_note="pending: window did not appear"
    fi
  else
    zed_note="pending: open -a Zed failed"
  fi
  osascript -e 'tell application "Zed" to quit' >/dev/null 2>&1 || true
else
  zed_note="pending: brew install --cask zed failed"
fi
printf '%s\n' "$subl_note" > "$out/sublime.txt"
printf '%s\n' "$zed_note" > "$out/zed.txt"

{
  echo "# Cold start"
  echo
  echo "Median of 10 release launches, process start to the first presented frame."
  echo "The median is the 5th of the 10 sorted samples, the same rule as \`scripts/p0-perf.sh\`."
  echo "Before is ${before_sha} (fonts deferred, adapter after the window)."
  echo "After overlaps the adapter request with window creation."
  echo "Run 43525490937 measured that before build at 246.6 ms."
  echo
  echo "Before: ${before_ms} ms."
  echo
  echo "After: ${after_ms} ms."
  echo
  echo "Budget: < 150 ms. A miss is recorded and does not fail the job."
  echo
  echo "Largest step on the 10th after launch: ${cause}."
  echo
  echo "Sublime Text: ${subl_note}."
  echo
  echo "Zed: ${zed_note}."
  echo
  echo "Phases (cumulative from process start, then the delta):"
  echo
  cat "$out/cause.txt"
  echo
  echo "Samples before (ms):"
  echo
  cat "$out/samples-before.txt"
  echo
  echo "Samples after (ms):"
  echo
  cat "$out/samples-after.txt"
} > "$out/h1.md"

echo "cold start before ${before_ms} ms after ${after_ms} ms cause ${cause}"
awk -v m="$after_ms" 'BEGIN {
  if (m + 0 >= 150) print "cold start budget miss: " m " ms"
  else print "cold start within budget: " m " ms"
}'
