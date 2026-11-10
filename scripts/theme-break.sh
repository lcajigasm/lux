#!/usr/bin/env bash
# macos-15 proof for a hot theme change, Cmd+N / Cmd+,, the settings
# page, and the palette. Pixel checks live in scripts/theme_pixels.py.
# A 1× runner cannot show an external Retina display; that case is
# recorded as solo-Mac and the scale-2 PNGs are still checked.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "theme-break captures run on macOS. This host is $(uname -s)." >&2
  exit 1
fi

out="$root/docs/evidence/themes"
rm -rf "$out"
mkdir -p "$out"
report="$out/report.txt"
: > "$report"
real_home="$HOME"
pass() { echo "PASS $1" | tee -a "$report"; }
fail() { echo "FAIL $1" | tee -a "$report" >&2; exit 1; }

echo "--> Building lux"
cargo build -p lux --locked
bin="$root/target/debug/lux"
app="$(bash "$root/scripts/stage-lux-app.sh" --no-build)"
echo "--> bundled app $app" | tee -a "$report"

fixture="$out/sample.rs"
{
  echo "fn theme_probe(value: i32) -> i32 {"
  for n in $(seq 1 40); do
    echo "    let item_$n = value + $n;"
  done
  echo "    item_1 + item_40"
  echo "}"
} > "$fixture"

theme_home="$out/home"
mkdir -p "$theme_home/.config/lux"
settings="$theme_home/.config/lux/settings.json"

write_theme() {
  local name="$1"
  python3 - "$settings" "$name" << 'PY'
import json, sys
path, name = sys.argv[1], sys.argv[2]
json.dump({
    "editor.fontSize": 13.0,
    "editor.lineHeight": 1.4,
    "workbench.colorTheme": name,
}, open(path, "w"), indent=2)
PY
}

snap() {
  local dest="$1"
  shift
  rm -f "$dest" "$dest.log"
  HOME="$theme_home" "$bin" "$@" --snapshot-scale 2 "$fixture" >"$dest.log" 2>&1 &
  local pid=$!
  local ready=0
  local _
  for _ in $(seq 1 45); do
    if [[ -s "$dest" ]]; then
      ready=1
      break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      break
    fi
    sleep 0.5
  done
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  if [[ "$ready" != 1 ]]; then
    echo "Lux did not write $dest" | tee -a "$report" >&2
    tail -40 "$dest.log" >&2 || true
    exit 1
  fi
}

slug() {
  printf '%s' "$1" | tr ' [:upper:]' '-[:lower:]' | tr -cd 'a-z0-9._-'
}

echo "--> Theme captures"
while IFS=$'\t' read -r name bg tab status panel row input; do
  [[ -n "$name" ]] || continue
  id="$(slug "$name")"
  write_theme "$name"
  editor="$out/${id}-editor.png"
  palette="$out/${id}-palette.png"
  page="$out/${id}-settings.png"
  snap "$editor" --snapshot "$editor"
  if [[ "$bg" == "any" ]]; then
    python3 "$root/scripts/theme_pixels.py" editor "$editor" any | tee -a "$report"
  else
    python3 "$root/scripts/theme_pixels.py" editor "$editor" "$bg" "$tab" "$status" | tee -a "$report"
    snap "$palette" --palette --query theme --snapshot "$palette"
    python3 "$root/scripts/theme_pixels.py" palette "$palette" "$row" | tee -a "$report"
    snap "$page" --settings --snapshot "$page"
    python3 "$root/scripts/theme_pixels.py" settings "$page" "$panel" | tee -a "$report"
  fi
  pass "theme $name"
done < <(python3 "$root/scripts/theme_pixels.py" list "$root/themes")

scale_line="$(grep -h "display scale" "$out"/*.log | awk '{print $NF}' | sort | uniq -c | sort -nr | head -1 || true)"
{
  echo "display-scale-counts: ${scale_line:-none}"
  system_profiler SPDisplaysDataType 2>/dev/null | sed -n '1,80p' || true
} | tee -a "$report" > "$out/scale.txt"
if grep -q "display scale 1" "$out"/*.log && ! grep -Eq "display scale ([2-9]|[0-9]{2})" "$out"/*.log; then
  echo "SCALE solo-Mac: the runner framebuffer is 1x. The scale-2 PNGs are the renderer, not an external HiDPI display." | tee -a "$report"
else
  echo "SCALE the runner reported a display scale other than 1. See scale.txt." | tee -a "$report"
fi
if ! grep -h "UI font" "$out"/*.log | head -5 | tee -a "$report"; then
  fail "the log never named a UI font"
fi
if grep -h "no system UI font" "$out"/*.log >/dev/null; then
  fail "selectors fell back to the editor face"
fi
if grep -h "UI font" "$out"/*.log | grep -Eq "Menlo|Monaco|DejaVu Sans Mono|Courier"; then
  fail "palette and settings used a monospace UI font"
fi

echo "--> Cmd+N and Cmd+,"
key_home="$out/key-home"
mkdir -p "$key_home/.config/lux"
printf '%s\n' '{ "editor.fontSize": 13.0, "editor.lineHeight": 1.4, "workbench.colorTheme": "monokai" }' \
  > "$key_home/.config/lux/settings.json"
export HOME="$key_home"
drive_init "$app"
trap drive_stop EXIT
drive_launch "$fixture"
drive_key tap 45 cmd
sleep 0.4
drive_dump
tabs="$(drive_field tabs)"
dirty="$(drive_field dirty)"
language="$(drive_field language)"
active="$(drive_field active_path)"
settings_open="$(drive_field settings)"
{
  echo "after Cmd+N tabs=$tabs dirty=$dirty language=$language settings=$settings_open path=$active"
} | tee -a "$report"
[[ "$tabs" -ge 2 ]] || fail "Cmd+N did not open a buffer (tabs=$tabs)"
[[ "$dirty" == "0" ]] || fail "the new buffer is dirty"
[[ "$language" == "Plain Text" ]] || fail "the new buffer language is $language"
[[ -z "$active" ]] || fail "the new buffer has a path: $active"
[[ "$settings_open" == "0" ]] || fail "Cmd+N opened Settings"
pass "Cmd+N opens a clean Plain Text buffer"
drive_key tap 43 cmd
sleep 0.4
drive_dump
settings_open="$(drive_field settings)"
echo "after Cmd+, settings=$settings_open" | tee -a "$report"
[[ "$settings_open" == "1" ]] || fail "Cmd+, did not open Settings"
pass "Cmd+, opens Settings"
drive_stop
trap - EXIT
export HOME="$real_home"

echo "--> Lux 1.5.6 settings, before this branch"
before_home="$out/before-home"
mkdir -p "$before_home/.config/lux"
dmg="$out/lux-1.5.6.dmg"
curl -fsSL -o "$dmg" \
  "https://github.com/lcajigasm/lux/releases/download/v1.5.6/lux-macos-universal.dmg"
mount="$out/mnt"
mkdir -p "$mount"
hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$dmg"
xattr -dr com.apple.quarantine "$mount/Lux.app" || true
before_bin="$mount/Lux.app/Contents/MacOS/lux"
[[ -x "$before_bin" ]] || fail "v1.5.6 dmg has no Lux.app binary"
HOME="$before_home" "$before_bin" --settings --snapshot "$out/settings-before.png" \
  --snapshot-scale 2 "$fixture" >"$out/settings-before.log" 2>&1 &
before_pid=$!
before_ready=0
for _ in $(seq 1 45); do
  if [[ -s "$out/settings-before.png" ]]; then
    before_ready=1
    break
  fi
  if ! kill -0 "$before_pid" 2>/dev/null; then
    break
  fi
  sleep 0.5
done
kill "$before_pid" 2>/dev/null || true
wait "$before_pid" 2>/dev/null || true
hdiutil detach "$mount" || true
[[ "$before_ready" == 1 ]] || fail "v1.5.6 did not write settings-before.png"
pass "captured Lux 1.5.6 settings"

echo "--> Zed settings"
if ! brew install --cask zed; then
  fail "brew could not install Zed"
fi
xattr -dr com.apple.quarantine "/Applications/Zed.app" || true
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
"$lsregister" -f "/Applications/Zed.app" || true
zed_id="$(defaults read "/Applications/Zed.app/Contents/Info.plist" CFBundleIdentifier 2>/dev/null || echo "dev.zed.Zed")"
for zed_support in "$real_home/Library/Application Support/Zed" "$real_home/.config/zed"; do
  mkdir -p "$zed_support"
  printf '%s\n' '{ "theme": "One Dark", "ui_font_size": 16, "buffer_font_size": 15 }' \
    > "$zed_support/settings.json"
done
open -a "/Applications/Zed.app" "$fixture" || open "/Applications/Zed.app" || true
zed_ready=0
for _ in $(seq 1 90); do
  if swift /tmp/lux-find-window.swift Zed >/dev/null 2>&1; then
    zed_ready=1
    break
  fi
  sleep 1
done
[[ "$zed_ready" == 1 ]] || fail "Zed did not open a window"
osascript -e "tell application id \"$zed_id\" to activate" || true
sleep 0.6
# Physical comma, same code the Lux chord test uses.
swift /tmp/lux-drive.swift tap 43 cmd
sleep 0.8
zed_box="$(swift /tmp/lux-find-window.swift Zed)"
zed_id_win="${zed_box%% *}"
# lux-find-window prints x y w h. Ask CoreGraphics for the window id.
cat > /tmp/lux-window-id.swift << 'SWIFT'
import CoreGraphics
import Foundation
let want = CommandLine.arguments.dropFirst().first ?? "Zed"
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] ?? []
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 || owner.caseInsensitiveCompare(want) != .orderedSame { continue }
    let id = (w[kCGWindowNumber as String] as? NSNumber)?.intValue ?? 0
    if id != 0 { print(id); exit(0) }
}
exit(1)
SWIFT
win="$(swift /tmp/lux-window-id.swift Zed)"
screencapture -l "$win" -o "$out/settings-zed.png" || true
if [[ ! -s "$out/settings-zed.png" ]]; then
  fail "screencapture produced no Zed settings image"
fi
osascript -e "tell application id \"$zed_id\" to quit" >/dev/null 2>&1 || true
pass "captured Zed settings"
cp "$out/monokai-settings.png" "$out/settings-after.png"
python3 "$root/scripts/theme_pixels.py" composite \
  "$out/settings-beside-zed.png" \
  "$out/settings-before.png" \
  "$out/settings-after.png" \
  "$out/settings-zed.png" | tee -a "$report"
pass "settings before, after, and Zed"
echo "--> theme-break done"
