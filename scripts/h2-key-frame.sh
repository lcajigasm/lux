#!/usr/bin/env bash
# Key event to the next presented frame, p99, on a 20k-line file.
# Fewer than 10 samples, a clock of 0, a file that did not take the
# keystrokes, or a missing frame fails the job. A budget miss does not.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/h2"
rm -rf "$out"
mkdir -p "$out"

bash "$root/scripts/stage-lux-app.sh" --release
drive_init "$root/target/Lux.app"

token="abcdefghijklmnopqrstuvwxyz0123456789ABCD"
big="$DRIVE_WORK/lines-20k.txt"
python3 - "$big" << 'PY'
import sys
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    for i in range(20000):
        handle.write(f"line {i}\n")
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

drive_launch "$big" --snapshot-scale 2 --snapshot-on-usr1 "$out/typed.png"
ready=0
for _ in $(seq 1 40); do
  drive_dump
  case "$(drive_field active_path)" in
    *lines-20k.txt) ready=1; break ;;
  esac
  sleep 0.25
done
if [[ "$ready" -ne 1 ]]; then
  echo "20k file did not open" >&2
  cp "$DRIVE_LOG" "$out/lux.log" || true
  exit 1
fi

drive_key chars "$token"
sleep 0.6
drive_dump
h2_us="$(drive_field key_p99_us)"
h2_n="$(drive_field key_n)"
echo "H2 p99 ${h2_us} us over ${h2_n} keys"
if [[ -z "$h2_n" || "$h2_n" -lt 10 || -z "$h2_us" || "$h2_us" -eq 0 ]]; then
  echo "H2 did not record a key-to-frame sample (n=${h2_n:-0} us=${h2_us:-0})" >&2
  cp "$DRIVE_PROBE" "$out/probe.txt" || true
  cp "$DRIVE_LOG" "$out/lux.log" || true
  exit 1
fi

drive_snap "$out/typed.png"
drive_save
saved=0
for _ in $(seq 1 24); do
  if grep -q "$token" "$big" && grep -q "line 19999" "$big"; then
    saved=1
    break
  fi
  sleep 0.25
done
if [[ "$saved" -ne 1 ]]; then
  echo "the 20k file does not contain the typed text" >&2
  exit 1
fi

python3 "$root/scripts/frame_png.py" frame "$out/typed.png"
python3 "$root/scripts/frame_png.py" ink "$out/typed.png"
cp "$DRIVE_PROBE" "$out/probe.txt"
cp "$DRIVE_LOG" "$out/lux.log"
drive_stop || true

h2_ms="$(python3 - "$h2_us" << 'PY'
import sys
print(f"{int(sys.argv[1]) / 1000:.2f}")
PY
)"

{
  echo "# H2"
  echo
  echo "Key event to the next presented frame, p99, on a 20,000-line file."
  echo
  echo "p99: ${h2_ms} ms (${h2_us} us) over ${h2_n} keys."
  echo
  echo "Budget: < 8 ms. A miss is recorded and does not fail the job."
  echo
  echo "The saved file starts with the typed text and still contains \`line 19999\`."
} > "$out/h2.md"

awk -v m="$h2_ms" 'BEGIN {
  if (m + 0 >= 8) print "H2 budget miss: " m " ms"
  else print "H2 within budget: " m " ms"
}'
echo "H2 p99 ${h2_ms} ms"
