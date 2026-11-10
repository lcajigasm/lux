#!/usr/bin/env bash
# macos-15: one shared overlay. Scroll the language picker, theme picker,
# and palette to the last row, apply the last language, and restart.
# Screenshots are the compiled app at snapshot scale 2.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "overlay-e2e runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

real_home="${HOME}"
out="$root/docs/evidence/overlay"
rm -rf "$out"
mkdir -p "$out"
report="$out/report.txt"
: > "$report"
pass() { echo "PASS $1" | tee -a "$report"; }
fail() {
  echo "FAIL $1" | tee -a "$report" >&2
  if [[ -n "${DRIVE_PROBE:-}" && -s "$DRIVE_PROBE" ]]; then
    echo "probe:" | tee -a "$report" >&2
    cat "$DRIVE_PROBE" | tee -a "$report" >&2
  fi
  exit 1
}

home="$out/home"
mkdir -p "$home/.config/lux"
fixture="$out/overlay-probe.txt"
printf 'overlay probe\nline two\n' > "$fixture"

echo "--> Building lux"
HOME="$real_home" cargo build -p lux --locked
app="$(HOME="$real_home" bash "$root/scripts/stage-lux-app.sh" --no-build)"
echo "--> bundled app $app" | tee -a "$report"

export HOME="$home"
drive_init "$app"
trap drive_stop EXIT

capture() {
  local dest="$1"
  kill -USR1 "$DRIVE_PID"
  local _
  for _ in $(seq 1 40); do
    if [[ -s "$dest" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "snapshot missing: $dest" | tee -a "$report" >&2
  tail -40 "$DRIVE_LOG" >&2 || true
  exit 1
}

open_palette() {
  local query="$1"
  drive_key tap 35 "cmd,shift"
  sleep 0.25
  if [[ -n "$query" ]]; then
    drive_key chars "$query"
    sleep 0.25
  fi
  drive_dump
  [[ "$(drive_field palette)" == "1" || "$(drive_field picker)" != "off" ]] || fail "palette did not open for '$query'"
}

# Wheel, Page Down, then End. The selection has to land on the last row.
scroll_to_end() {
  local kind="$1"
  drive_scroll -4
  sleep 0.25
  drive_dump
  [[ "$(drive_field picker)" == "$kind" ]] || fail "$kind picker is $(drive_field picker) after the wheel"
  [[ "$(drive_field picker_index)" != "0" ]] || fail "$kind wheel left the selection on the first row"
  drive_key tap 115 ""
  sleep 0.15
  drive_key tap 121 ""
  sleep 0.2
  drive_dump
  local paged
  paged="$(drive_field picker_index)"
  [[ "$paged" -gt 1 ]] || fail "$kind Page Down stopped at $paged"
  drive_key tap 119 ""
  sleep 0.25
  drive_dump
  local index count
  index="$(drive_field picker_index)"
  count="$(drive_field picker_count)"
  [[ "$count" -gt 1 ]] || fail "$kind has $count rows"
  [[ "$index" -eq $((count - 1)) ]] || fail "$kind End landed on $index of $count"
  [[ -n "$(drive_field picker_label)" ]] || fail "$kind last row has no label"
  echo "$kind index=$index count=$count label=$(drive_field picker_label)" | tee -a "$report"
  pass "$kind scrolls to the last row"
}

drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/palette-monokai.png" \
  --snapshot-on-usr1 "$out/language-monokai.png" \
  --snapshot-on-usr1 "$out/theme-monokai.png" \
  --snapshot-on-usr1 "$out/goto-monokai.png" \
  --snapshot-on-usr1 "$out/palette-solarized-light.png" \
  --snapshot-on-usr1 "$out/language-solarized-light.png" \
  --snapshot-on-usr1 "$out/theme-solarized-light.png" \
  --snapshot-on-usr1 "$out/goto-solarized-light.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
[[ "$opened" == 1 ]] || fail "probe file never became active"
[[ "$(drive_field theme)" == "Monokai" ]] || fail "opened on $(drive_field theme)"

open_palette ""
capture "$out/palette-monokai.png"
scroll_to_end palette
drive_key tap 53 ""
sleep 0.25

open_palette "language mode"
drive_key tap 36 ""
sleep 0.35
drive_dump
[[ "$(drive_field picker)" == "language" ]] || fail "language picker did not open ($(drive_field picker_label))"
[[ "$(drive_field picker_count)" -gt 8 ]] || fail "language picker still shows only $(drive_field picker_count) rows"
scroll_to_end language
capture "$out/language-monokai.png"
chosen="$(drive_field picker_label)"
drive_key tap 36 ""
sleep 0.4
drive_dump
[[ "$(drive_field language)" == "$chosen" ]] || fail "applied $(drive_field language), want $chosen"
[[ "$(drive_field picker)" == "off" ]] || fail "language picker stayed open"
pass "last language applied ($chosen)"

open_palette "color theme"
drive_key tap 36 ""
sleep 0.35
drive_dump
[[ "$(drive_field picker)" == "theme" ]] || fail "theme picker did not open"
capture "$out/theme-monokai.png"
scroll_to_end theme
drive_key tap 53 ""
sleep 0.3
drive_dump
[[ "$(drive_field theme)" == "Monokai" ]] || fail "Esc applied $(drive_field theme)"

open_palette "go to line"
drive_key tap 36 ""
sleep 0.35
drive_dump
[[ "$(drive_field picker)" == "goto" || "$(drive_field goto)" == "1" ]] || fail "go to line did not open"
capture "$out/goto-monokai.png"
drive_key tap 53 ""
sleep 0.25

open_palette "color theme"
drive_key tap 36 ""
sleep 0.3
drive_key chars "solarized light"
sleep 0.3
drive_key tap 36 ""
sleep 0.45
drive_dump
[[ "$(drive_field theme)" == "Solarized Light" ]] || fail "theme is $(drive_field theme)"
pass "Solarized Light"

open_palette ""
capture "$out/palette-solarized-light.png"
scroll_to_end palette
drive_key tap 53 ""
sleep 0.25

open_palette "language mode"
drive_key tap 36 ""
sleep 0.35
scroll_to_end language
capture "$out/language-solarized-light.png"
drive_key tap 53 ""
sleep 0.25
drive_dump
[[ "$(drive_field language)" == "$chosen" ]] || fail "Esc changed the language to $(drive_field language)"

open_palette "color theme"
drive_key tap 36 ""
sleep 0.35
capture "$out/theme-solarized-light.png"
scroll_to_end theme
drive_key tap 53 ""
sleep 0.3

open_palette "go to line"
drive_key tap 36 ""
sleep 0.35
capture "$out/goto-solarized-light.png"
drive_key tap 53 ""
sleep 0.2

drive_key tap 12 cmd
for _ in $(seq 1 40); do
  if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
    wait "$DRIVE_PID" 2>/dev/null || true
    DRIVE_PID=""
    break
  fi
  sleep 0.25
done
[[ -z "${DRIVE_PID:-}" ]] || fail "Cmd+Q did not exit"
pass "clean quit"

drive_launch "$fixture"
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    break
  fi
  sleep 0.3
done
drive_dump
[[ "$(drive_field language)" == "$chosen" ]] || fail "restart language is $(drive_field language), want $chosen"
[[ "$(drive_field theme)" == "Solarized Light" ]] || fail "restart theme is $(drive_field theme)"
pass "language and theme persisted"
drive_stop
trap - EXIT

if grep -h "no system UI font" "$DRIVE_LOG" >/dev/null; then
  fail "selectors fell back to the editor face"
fi
if grep -h "UI font" "$DRIVE_LOG" | grep -Eq "Menlo|Monaco|DejaVu Sans Mono|Courier"; then
  fail "overlay used a monospace UI font"
fi
pass "system UI font"
echo "--> overlay-e2e done"
