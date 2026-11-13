#!/usr/bin/env bash
# Slowest redraw while scrolling a 50k-line file. Fewer than five frames,
# a viewport that did not move, two identical captures, or a clock of 0
# fails the job. A budget miss does not.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/h5"
rm -rf "$out"
mkdir -p "$out"

bash "$root/scripts/stage-lux-app.sh" --release
drive_init "$root/target/Lux.app"

scroll="$DRIVE_WORK/lines-50k.txt"
python3 - "$scroll" << 'PY'
import sys
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    for i in range(50000):
        handle.write(f"line {i} ....\n")
PY

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

drive_launch "$scroll" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/top.png" \
  --snapshot-on-usr1 "$out/scrolled.png"

ready=0
for _ in $(seq 1 40); do
  drive_dump
  case "$(drive_field active_path)" in
    *lines-50k.txt) ready=1; break ;;
  esac
  sleep 0.25
done
if [[ "$ready" -ne 1 ]]; then
  echo "50k file did not open" >&2
  cp "$DRIVE_LOG" "$out/lux.log" || true
  exit 1
fi

before="$(drive_field scroll_y)"
drive_snap "$out/top.png"
for _ in $(seq 1 40); do
  drive_scroll
  sleep 0.05
done
sleep 0.4
drive_snap "$out/scrolled.png"
drive_dump
after="$(drive_field scroll_y)"
h5_us="$(drive_field frame_max_us)"
h5_n="$(drive_field frame_n)"
echo "H5 max ${h5_us} us over ${h5_n} frames, scroll_y ${before} -> ${after}"
if [[ -z "$h5_n" || "$h5_n" -lt 5 || -z "$h5_us" || "$h5_us" -eq 0 ]]; then
  echo "H5 did not record a scroll frame (n=${h5_n:-0} us=${h5_us:-0})" >&2
  cp "$DRIVE_PROBE" "$out/probe.txt" || true
  cp "$DRIVE_LOG" "$out/lux.log" || true
  exit 1
fi
if [[ -z "$before" || -z "$after" || "$after" -le "$before" ]]; then
  echo "scroll_y did not move (${before:-missing} -> ${after:-missing})" >&2
  exit 1
fi

python3 "$root/scripts/frame_png.py" frame "$out/top.png"
python3 "$root/scripts/frame_png.py" frame "$out/scrolled.png"
python3 "$root/scripts/frame_png.py" differ "$out/top.png" "$out/scrolled.png"
cp "$DRIVE_PROBE" "$out/probe.txt"
cp "$DRIVE_LOG" "$out/lux.log"
drive_stop || true

h5_ms="$(python3 - "$h5_us" << 'PY'
import sys
print(f"{int(sys.argv[1]) / 1000:.2f}")
PY
)"

{
  echo "# H5"
  echo
  echo "Slowest redraw while scrolling a 50,000-line file."
  echo
  echo "Max: ${h5_ms} ms (${h5_us} us) over ${h5_n} frames."
  echo
  echo "scroll_y ${before} -> ${after}."
  echo
  echo "Budget: no frame > 16 ms. A miss is recorded and does not fail the job."
} > "$out/h5.md"

awk -v m="$h5_ms" 'BEGIN {
  if (m + 0 > 16) print "H5 budget miss: " m " ms"
  else print "H5 within budget: " m " ms"
}'
echo "H5 max ${h5_ms} ms"
