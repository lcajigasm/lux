#!/usr/bin/env bash
# Extra RSS while opening a 100 MB file. Main and this branch, one macos-15 run.
# A missing clock, a missing RSS sample, or a missing frame fails the job.
# A budget miss is written down and does not.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/h6-rss"
rm -rf "$out"
mkdir -p "$out"
raw="$out/raw.txt"
: > "$raw"
log() { echo "$*" | tee -a "$raw"; }

main_sha="2b61fc366a3a6a370bb034fe62a33d0856054334"
bytes=$((100 * 1024 * 1024))

bash "$root/scripts/stage-lux-app.sh" --release
cp -R "$root/target/Lux.app" /tmp/Lux-after.app

git fetch --depth 1 origin "$main_sha"
git worktree add --detach /tmp/lux-main "$main_sha"
(
  cd /tmp/lux-main
  CARGO_TARGET_DIR="$root/target" cargo build -p lux --release --locked
)
mkdir -p /tmp/lux-main/target/release
cp "$root/target/release/lux" /tmp/lux-main/target/release/lux
(cd /tmp/lux-main && bash scripts/stage-lux-app.sh --release --no-build)
cp -R /tmp/lux-main/target/Lux.app /tmp/Lux-before.app

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
  tail -40 "$DRIVE_LOG" >&2 || true
  return 1
}

measure() {
  local label="$1"
  local big="$DRIVE_WORK/open.txt"
  local small="$DRIVE_WORK/warm.txt"
  python3 - "$big" "$bytes" << 'PY'
import sys
path, nbytes = sys.argv[1], int(sys.argv[2])
line = b"line\n"
with open(path, "wb") as handle:
    written = 0
    while written + len(line) <= nbytes:
        handle.write(line)
        written += len(line)
    if written < nbytes:
        handle.write(b"x" * (nbytes - written))
PY
  printf 'warm\n' > "$small"
  if [[ "$label" == "after" ]]; then
    drive_launch "$small" --bench-open "$big" --snapshot-scale 2 --snapshot-on-usr1 "$out/opened.png"
  else
    drive_launch "$small" --bench-open "$big"
  fi
  local ms0=""
  local _
  for _ in $(seq 1 40); do
    ms0="$(awk '/^Lux: first frame [0-9.]+ ms$/ { print $4; exit }' "$DRIVE_LOG")"
    if [[ -n "$ms0" ]]; then
      break
    fi
    sleep 0.25
  done
  if [[ -z "$ms0" ]]; then
    echo "$label warm start produced no first frame" >&2
    cp "$DRIVE_LOG" "$out/${label}.log" || true
    exit 1
  fi
  sleep 0.4
  local idle hot extra ratio ms
  idle="$(ps -o rss= -p "$DRIVE_PID" | tr -d ' ')"
  kill -USR2 "$DRIVE_PID"
  ms=""
  for _ in $(seq 1 120); do
    if grep -q 'Lux: buffer frame' "$DRIVE_LOG"; then
      ms="$(grep 'Lux: buffer frame' "$DRIVE_LOG" | tail -1 | awk '{print $(NF-1)}')"
      break
    fi
    if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
      echo "$label exited before the buffer frame" >&2
      cp "$DRIVE_LOG" "$out/${label}.log" || true
      exit 1
    fi
    sleep 0.5
  done
  if [[ -z "$ms" ]]; then
    echo "$label has no buffer frame" >&2
    cp "$DRIVE_LOG" "$out/${label}.log" || true
    exit 1
  fi
  sleep 0.3
  hot="$(ps -o rss= -p "$DRIVE_PID" | tr -d ' ')"
  extra=$((hot - idle))
  ratio="$(python3 - "$extra" "$bytes" << 'PY'
import sys
extra_kb, nbytes = int(sys.argv[1]), int(sys.argv[2])
print(f"{(extra_kb * 1024) / nbytes:.2f}")
PY
)"
  printf '%s\n' "$ms" > "$out/${label}-ms.txt"
  printf '%s\n' "$ratio" > "$out/${label}-ratio.txt"
  printf '%s %s\n' "$idle" "$hot" > "$out/${label}-rss.txt"
  log "$label open ${ms} ms idle_rss_kb ${idle} hot_rss_kb ${hot} extra_rss_kb ${extra} ratio ${ratio}"
  if [[ "$label" == "after" ]]; then
    drive_snap "$out/opened.png"
    cp "$DRIVE_LOG" "$out/after.log"
  else
    cp "$DRIVE_LOG" "$out/before.log"
  fi
  drive_stop || true
}

drive_init /tmp/Lux-before.app
measure before
drive_init /tmp/Lux-after.app
measure after

python3 "$root/scripts/frame_png.py" frame "$out/opened.png"
python3 "$root/scripts/frame_png.py" ink "$out/opened.png"

before_ms="$(cat "$out/before-ms.txt")"
after_ms="$(cat "$out/after-ms.txt")"
before_ratio="$(cat "$out/before-ratio.txt")"
after_ratio="$(cat "$out/after-ratio.txt")"

python3 - "$out/after.log" "$out/phases.txt" << 'PY'
import re
import sys
from pathlib import Path
text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
found = {}
for name, ms in re.findall(r"Lux: open phase (\w+) ([0-9.]+) ms", text):
    found[name] = float(ms)
frame = re.search(r"Lux: buffer frame ([0-9.]+) ms", text)
points = [(name, found[name]) for name in ("read", "apply") if name in found]
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
body = "\n".join(lines) + f"\ncause {cause} {cause_ms:.1f} ms\n"
Path(sys.argv[2]).write_text(body, encoding="utf-8")
PY
cause="$(awk '/^cause / { print $2 " " $3; exit }' "$out/phases.txt")"

export HOMEBREW_NO_AUTO_UPDATE=1
export NONINTERACTIVE=1
subl_note="pending"
zed_note="pending"
log "--> Sublime and Zed, official cask, window listed"
if brew install --cask sublime-text >> "$raw" 2>&1; then
  subl="$(command -v subl || true)"
  if [[ -n "$subl" ]]; then
    "$subl" -n "$DRIVE_WORK/warm.txt" >/dev/null 2>&1 || true
    if swift /tmp/lux-find-window.swift "Sublime Text" >/dev/null 2>&1; then
      subl_note="window listed"
      log "Sublime Text window listed"
    else
      subl_note="pending: window did not appear"
      log "Sublime Text window did not appear"
    fi
    osascript -e 'tell application "Sublime Text" to quit' >/dev/null 2>&1 || true
  else
    subl_note="pending: subl not on PATH after the cask install"
    log "subl not on PATH"
  fi
else
  subl_note="pending: brew install --cask sublime-text failed"
  log "brew install --cask sublime-text failed"
fi
if brew install --cask zed >> "$raw" 2>&1; then
  if open -a "Zed" "$DRIVE_WORK/warm.txt" >> "$raw" 2>&1; then
    if swift /tmp/lux-find-window.swift Zed >/dev/null 2>&1; then
      zed_note="window listed"
      log "Zed window listed"
    else
      zed_note="pending: window did not appear"
      log "Zed window did not appear"
    fi
  else
    zed_note="pending: open -a Zed failed"
    log "open -a Zed failed"
  fi
  osascript -e 'tell application "Zed" to quit' >/dev/null 2>&1 || true
else
  zed_note="pending: brew install --cask zed failed"
  log "brew install --cask zed failed"
fi
printf '%s\n' "$subl_note" > "$out/sublime.txt"
printf '%s\n' "$zed_note" > "$out/zed.txt"

{
  echo "# H6 extra RSS, 100 MB"
  echo
  echo "Warm process, SIGUSR2 until the opened buffer's first presented frame."
  echo "Main \`$main_sha\` and this branch were measured in this run."
  echo "The starting figure on run 43471189705 was 4.84× extra RSS, with the open at 9049.3 ms."
  echo
  echo "Before (main): extra RSS ${before_ratio}× (${before_ms} ms to open)."
  echo
  echo "After: extra RSS ${after_ratio}× (${after_ms} ms to open)."
  echo
  echo "Budget: extra RSS < 3× the file. A miss is recorded and does not fail the job."
  echo
  echo "Largest step on the after open: ${cause}."
  echo
  echo "Phases (cumulative from SIGUSR2, then the delta):"
  echo
  cat "$out/phases.txt"
  echo
  echo "Sublime Text: ${subl_note}."
  echo
  echo "Zed: ${zed_note}."
} > "$out/h6.md"

log "H6 before ${before_ratio}x after ${after_ratio}x open ${before_ms} -> ${after_ms} ms"
awk -v m="$after_ms" 'BEGIN {
  if (m + 0 >= 150) print "H3 budget miss: " m " ms"
  else print "H3 within budget: " m " ms"
}'
