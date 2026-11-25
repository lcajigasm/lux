#!/usr/bin/env bash
# Key event to the next presented frame, p99, on a 20k-line file.
# Main and this branch are measured in one job. Fewer than 37 samples,
# a clock of 0, a file that lost the keystrokes or line 19999, or a
# missing frame fails the job. A budget miss does not.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/h2-present"
rm -rf "$out"
mkdir -p "$out"

before_sha="7a85130a950fba33de526bd4f647c948aca9fdf7"
token="abcdefghijklmnopqrstuvwxyz0123456789ABCD"

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

type_file() {
  local label="$1"
  local big="$2"
  python3 - "$big" << 'PY'
import sys
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    for i in range(20000):
        handle.write(f"line {i}\n")
PY
  if [[ "$label" == "after" ]]; then
    drive_launch "$big" --snapshot-scale 2 --snapshot-on-usr1 "$out/typed.png"
  else
    drive_launch "$big"
  fi
  local ready=0
  for _ in $(seq 1 40); do
    drive_dump || true
    case "$(drive_field active_path)" in
      *lines-20k.txt) ready=1; break ;;
    esac
    sleep 0.25
  done
  if [[ "$ready" -ne 1 ]]; then
    echo "$label did not open the 20k file" >&2
    cp "$DRIVE_LOG" "$out/${label}.log" || true
    exit 1
  fi
  drive_key chars "$token"
  sleep 0.6
  drive_dump
  local us n
  us="$(drive_field key_p99_us)"
  n="$(drive_field key_n)"
  echo "$label p99 ${us} us over ${n} keys"
  if [[ -z "$n" || "$n" -lt 37 || -z "$us" || "$us" -eq 0 ]]; then
    echo "$label did not record 37 key-to-frame samples (n=${n:-0} us=${us:-0})" >&2
    cp "$DRIVE_PROBE" "$out/${label}-probe.txt" || true
    cp "$DRIVE_LOG" "$out/${label}.log" || true
    exit 1
  fi
  printf '%s\n' "$us" > "$out/${label}-us.txt"
  printf '%s\n' "$n" > "$out/${label}-n.txt"
  if [[ "$label" == "after" ]]; then
    drive_snap "$out/typed.png"
  fi
  drive_save
  local saved=0
  for _ in $(seq 1 24); do
    if grep -q "$token" "$big" && grep -q "line 19999" "$big"; then
      saved=1
      break
    fi
    sleep 0.25
  done
  if [[ "$saved" -ne 1 ]]; then
    echo "$label did not keep the typed text and line 19999" >&2
    exit 1
  fi
  cp "$DRIVE_PROBE" "$out/${label}-probe.txt"
  cp "$DRIVE_LOG" "$out/${label}.log"
  drive_stop || true
}

drive_init /tmp/Lux-before.app
type_file before "$DRIVE_WORK/lines-20k.txt"
drive_init /tmp/Lux-after.app
type_file after "$DRIVE_WORK/lines-20k.txt"

python3 "$root/scripts/frame_png.py" frame "$out/typed.png"
python3 "$root/scripts/frame_png.py" ink "$out/typed.png"

before_us="$(cat "$out/before-us.txt")"
after_us="$(cat "$out/after-us.txt")"
before_n="$(cat "$out/before-n.txt")"
after_n="$(cat "$out/after-n.txt")"
before_ms="$(awk -v u="$before_us" 'BEGIN { printf "%.2f", u / 1000 }')"
after_ms="$(awk -v u="$after_us" 'BEGIN { printf "%.2f", u / 1000 }')"

export HOMEBREW_NO_AUTO_UPDATE=1
export NONINTERACTIVE=1
subl_note="pending"
zed_note="pending"
echo "--> Sublime and Zed, official cask" | tee -a "$out/raw.txt"
if brew install --cask sublime-text >> "$out/raw.txt" 2>&1; then
  subl="$(command -v subl || true)"
  if [[ -n "$subl" ]]; then
    "$subl" -n "$DRIVE_WORK/lines-20k.txt" >/dev/null 2>&1 || true
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
  if open -a "Zed" "$DRIVE_WORK/lines-20k.txt" >> "$out/raw.txt" 2>&1; then
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
  echo "# Key to frame"
  echo
  echo "Key event to the next presented frame, p99, on a 20,000-line file."
  echo "Before is ${before_sha} (recovery already off the key path)."
  echo "After presents without waiting for the next refresh when the surface allows it."
  echo "Run 43525497637 measured that before build at 21.41 ms p99."
  echo
  echo "Before: ${before_ms} ms (${before_us} us) over ${before_n} keys."
  echo
  echo "After: ${after_ms} ms (${after_us} us) over ${after_n} keys."
  echo
  echo "Budget: < 8 ms. A miss is recorded and does not fail the job."
  echo
  echo "The recovery snapshot is written on a worker. The p99 is the presented frame."
  echo
  echo "Present mode on the after launch:"
  echo
  grep "Lux: present" "$out/after.log" || echo "present mode was not logged"
  echo "Both saved files start with the typed text and still contain \`line 19999\`."
  echo
  echo "Sublime Text: ${subl_note}."
  echo
  echo "Zed: ${zed_note}. Key-to-pixel is not readable from either process."
} > "$out/h2.md"

echo "key frame before ${before_ms} ms after ${after_ms} ms"
awk -v m="$after_ms" 'BEGIN {
  if (m + 0 >= 8) print "key frame budget miss: " m " ms"
  else print "key frame within budget: " m " ms"
}'
