#!/usr/bin/env bash
# macos-15: open Settings, scroll to the end, cycle ten themes, restart.
# Screenshots are the compiled app at snapshot scale 2.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "settings-e2e runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

real_home="${HOME}"
out="$root/docs/evidence/settings"
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
fixture="$out/sample.rs"
{
  # Stay under the probe's 2000-character text dump so the job can
  # compare the buffer after each theme. Fifty short lines still
  # overflow a 700px window.
  echo "fn theme_probe(value: i32) -> i32 {"
  for n in $(seq 1 50); do
    echo "    let n = 1;"
  done
  echo "    n"
  echo "}"
} > "$fixture"

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

drive_launch "$fixture" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/settings-monokai.png" \
  --snapshot-on-usr1 "$out/palette-monokai.png" \
  --snapshot-on-usr1 "$out/settings-solarized-light.png" \
  --snapshot-on-usr1 "$out/palette-solarized-light.png" \
  --snapshot-on-usr1 "$out/color-theme-chips.png" \
  --snapshot-on-usr1 "$out/color-theme-chips-scrolled.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
[[ "$opened" == 1 ]] || fail "sample.rs never became active"
[[ "$(drive_field theme)" == "Monokai" ]] || fail "opened on $(drive_field theme)"

drive_dump
before_sel="$(drive_field sel)"
before_y="$(drive_field scroll_y)"
drive_key tap 125 ""
drive_key tap 125 ""
drive_key tap 124 ""
# Page Down. CGEvent returns before the editor applies it. A fixed
# sleep has recorded the caret from the two Down arrows (52:52,
# scroll 0) and then failed when Page Down landed during the theme
# command (532:532, scroll 40). Poll until both have moved.
drive_key tap 121 ""
settled=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump
  sel="$(drive_field sel)"
  scroll_y="$(drive_field scroll_y)"
  if [[ "$sel" != "$before_sel" && "$scroll_y" != "$before_y" ]]; then
    settled=1
    break
  fi
done
[[ "$settled" == 1 ]] || fail "Page Down did not apply (sel=$sel scroll_y=$scroll_y, before $before_sel/$before_y)"
text="$(drive_field text)"
scroll_x="$(drive_field scroll_x)"
[[ "$text" == *theme_probe* ]] || fail "buffer text is missing the fixture"
[[ -n "$sel" ]] || fail "no cursor"
echo "cursor=$sel scroll_y=$scroll_y scroll_x=$scroll_x" | tee -a "$report"

drive_key tap 43 cmd
sleep 0.4
drive_dump
[[ "$(drive_field settings)" == "1" ]] || fail "Cmd+, did not open Settings"
pass "Cmd+, opens Settings"
capture "$out/settings-monokai.png"
drive_scroll -6
sleep 0.2
drive_key tap 119 ""
sleep 0.3
drive_dump
scrolled="$(drive_field settings_scroll)"
echo "settings_scroll=$scrolled" | tee -a "$report"
[[ "$scrolled" != "0" ]] || fail "Settings did not scroll"
pass "Settings scrolls to the end"
drive_key tap 53 ""
sleep 0.3
drive_dump
[[ "$(drive_field settings)" == "0" ]] || fail "Esc left Settings open"

drive_key tap 35 "cmd,shift"
sleep 0.3
drive_key chars "th"
sleep 0.3
drive_dump
[[ "$(drive_field palette)" == "1" ]] || fail "Cmd+Shift+P did not open the palette"
pass "palette is open"
capture "$out/palette-monokai.png"
drive_key tap 53 ""
sleep 0.3
drive_dump
[[ "$(drive_field palette)" == "0" ]] || fail "Esc left the palette open"
pass "Esc closes the palette"

apply_theme() {
  local query="$1"
  local expect="$2"
  drive_key tap 35 "cmd,shift"
  sleep 0.25
  drive_key chars "color theme"
  sleep 0.25
  drive_key tap 36 ""
  sleep 0.35
  drive_key chars "$query"
  sleep 0.3
  drive_key tap 36 ""
  sleep 0.45
  drive_dump
  local theme now_text now_sel now_y now_x
  theme="$(drive_field theme)"
  now_text="$(drive_field text)"
  now_sel="$(drive_field sel)"
  now_y="$(drive_field scroll_y)"
  now_x="$(drive_field scroll_x)"
  [[ "$theme" == "$expect" ]] || fail "$query applied $theme, want $expect"
  [[ "$now_text" == "$text" ]] || fail "$expect changed the buffer"
  [[ "$now_sel" == "$sel" ]] || fail "$expect moved the cursor ($now_sel, want $sel)"
  [[ "$now_y" == "$scroll_y" ]] || fail "$expect moved scroll_y ($now_y, want $scroll_y)"
  [[ "$now_x" == "$scroll_x" ]] || fail "$expect moved scroll_x ($now_x, want $scroll_x)"
  [[ "$(drive_field palette)" == "0" ]] || fail "palette stayed open after $expect"
  pass "$expect"
}

apply_theme "monokai" "Monokai"
apply_theme "one dark" "One Dark"
apply_theme "dracula" "Dracula"
apply_theme "solarized dark" "Solarized Dark"
apply_theme "nord" "Nord"
apply_theme "gruvbox" "Gruvbox"
apply_theme "tokyo" "Tokyo Night"
apply_theme "catppuccin" "Catppuccin"
apply_theme "latte" "Catppuccin Latte"
apply_theme "solarized light" "Solarized Light"
pass "ten themes kept the buffer, cursor, and scroll"

opened_settings=0
for _ in $(seq 1 4); do
  drive_key tap 43 cmd
  sleep 0.4
  drive_dump
  if [[ "$(drive_field settings)" == "1" ]]; then
    opened_settings=1
    break
  fi
done
[[ "$opened_settings" == 1 ]] || fail "Settings did not open on Solarized Light"
capture "$out/settings-solarized-light.png"
drive_key tap 53 ""
sleep 0.25
drive_key tap 35 "cmd,shift"
sleep 0.3
drive_key chars "th"
sleep 0.3
capture "$out/palette-solarized-light.png"
drive_key tap 53 ""
sleep 0.25
opened_chips=0
for _ in $(seq 1 4); do
  drive_key tap 43 cmd
  sleep 0.4
  drive_dump
  if [[ "$(drive_field settings)" == "1" ]]; then
    opened_chips=1
    break
  fi
done
[[ "$opened_chips" == 1 ]] || fail "Settings did not open for the color menu"
drive_key chars "color"
sleep 0.35
drive_key tap 36 ""
sleep 0.45
drive_dump
[[ "$(drive_field settings)" == "1" ]] || fail "color filter left Settings"
capture "$out/color-theme-chips.png"
[[ -s "$out/color-theme-chips.png" ]] || fail "color-theme-chips.png was not written"
[[ "$(drive_field settings_menu_count)" -gt 1 ]] || fail "color menu did not open"
for _ in $(seq 1 40); do
  if [[ "$(drive_field settings_menu)" -eq $(($(drive_field settings_menu_count) - 1)) ]]; then
    break
  fi
  drive_key tap 125 ""
  sleep 0.08
  drive_dump || true
done
[[ "$(drive_field settings_menu)" -eq $(($(drive_field settings_menu_count) - 1)) ]] \
  || fail "color menu did not reach the last theme"
[[ "$(drive_field settings_menu_first)" -gt 0 ]] || fail "color menu did not scroll"
capture "$out/color-theme-chips-scrolled.png"
[[ -s "$out/color-theme-chips-scrolled.png" ]] || fail "scrolled color menu was not written"
pass "color theme menu shows the chips and scrolls"
drive_key tap 53 ""
sleep 0.2
python3 "$root/scripts/gutter_pixels.py" --margin \
  "$out/palette-monokai.png" \
  "$out/palette-solarized-light.png" \
  || fail "line numbers are clipped"
pass "line numbers keep a 1px margin"
drive_key tap 53 ""
sleep 0.2

drive_stop
trap - EXIT
drive_launch "$fixture"
trap drive_stop EXIT
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    break
  fi
  sleep 0.3
done
drive_dump
[[ "$(drive_field theme)" == "Solarized Light" ]] || fail "restart theme is $(drive_field theme)"
[[ "$(drive_field text)" == *theme_probe* ]] || fail "restart lost the buffer"
pass "theme persisted after restart"
drive_stop
trap - EXIT

if grep -h "no system UI font" "$DRIVE_LOG" >/dev/null; then
  fail "selectors fell back to the editor face"
fi
if grep -h "UI font" "$DRIVE_LOG" | grep -Eq "Menlo|Monaco|DejaVu Sans Mono|Courier"; then
  fail "palette and settings used a monospace UI font"
fi
pass "system UI font"
echo "--> settings-e2e done"
