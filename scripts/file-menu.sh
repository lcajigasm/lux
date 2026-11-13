#!/usr/bin/env bash
# Open the real File menu on Lux.app and capture it. Fails if Open Recent,
# New Window, Save All, Reopen Closed Tab, or Clear Recent is missing, or if
# New Window is not Shift-Command-N.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/file-menu"
rm -rf "$out"
mkdir -p "$out"

export HOME
HOME="$(mktemp -d)"
unset XDG_CONFIG_HOME || true

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT

sample="$DRIVE_WORK/file-menu-sample.rs"
printf 'fn main() {}\n' > "$sample"
drive_launch "$sample"

ready=0
for _ in $(seq 1 40); do
  drive_dump
  if drive_field active_path | grep -q 'file-menu-sample.rs'; then
    ready=1
    break
  fi
  sleep 0.25
done
if [[ "$ready" != 1 ]]; then
  echo "file-menu-sample.rs never became the active path" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi

items="$out/menu-items.txt"
recent="$out/recent-items.txt"
shortcut="$out/new-window-shortcut.txt"
shot="$out/file-menu.png"
subshot="$out/file-menu-recent.png"
err="$out/menu.err"

if ! osascript <<EOF >"$err" 2>&1
tell application "System Events"
  tell (first process whose unix id is $DRIVE_PID)
    set frontmost to true
    click menu bar item "File" of menu bar 1
    delay 0.6
    set fileMenu to menu "File" of menu bar item "File" of menu bar 1
    set fileNames to ""
    repeat with menuItem in menu items of fileMenu
      try
        set itemName to name of menuItem
        if itemName is not missing value then
          set fileNames to fileNames & itemName & linefeed
        end if
      end try
    end repeat
    set recentMenu to menu "Open Recent" of menu item "Open Recent" of fileMenu
    set recentNames to ""
    repeat with menuItem in menu items of recentMenu
      try
        set itemName to name of menuItem
        if itemName is not missing value then
          set recentNames to recentNames & itemName & linefeed
        end if
      end try
    end repeat
    set nwChar to ""
    set nwMods to ""
    try
      set nw to menu item "New Window" of fileMenu
      set nwChar to (value of attribute "AXMenuItemCmdChar" of nw) as text
    end try
    try
      set nwMods to (value of attribute "AXMenuItemCmdModifiers" of nw) as text
    end try
    set shortcutLine to nwChar & linefeed & nwMods
    do shell script "printf %s " & quoted form of fileNames & " > " & quoted form of "$items"
    do shell script "printf %s " & quoted form of recentNames & " > " & quoted form of "$recent"
    do shell script "printf %s " & quoted form of shortcutLine & " > " & quoted form of "$shortcut"
    do shell script "screencapture -x " & quoted form of "$shot"
    click menu item "Open Recent" of fileMenu
    delay 0.5
    do shell script "screencapture -x " & quoted form of "$subshot"
  end tell
end tell
EOF
then
  echo "System Events could not open the File menu" >&2
  cat "$err" >&2 || true
  exit 1
fi

fail() {
  echo "$1" >&2
  echo "File menu:" >&2
  cat "$items" >&2 || true
  echo "Open Recent:" >&2
  cat "$recent" >&2 || true
  exit 1
}

for name in "New Window" "Open Recent" "Save All" "Reopen Closed Tab"; do
  grep -F -q "$name" "$items" || fail "File menu is missing $name"
done
# Command is the default. Shift is bit 1. Option, Control, and No Command
# (2, 4, 8) must stay off, so the mask is exactly 1: Shift-Command-N.
nw_char="$(sed -n '1p' "$shortcut" | tr '[:upper:]' '[:lower:]')"
nw_mods="$(sed -n '2p' "$shortcut" | tr -d '[:space:]')"
if [[ "$nw_char" != "n" || "$nw_mods" != "1" ]]; then
  echo "New Window shortcut:" >&2
  cat "$shortcut" >&2 || true
  fail "New Window is not Shift-Command-N (char=$nw_char mods=$nw_mods)"
fi
grep -F -q "file-menu-sample.rs" "$recent" || fail "Open Recent does not list the opened file"
grep -F -q "Clear Recent" "$recent" || fail "Open Recent is missing Clear Recent"
if grep -F -q "No Recent Files" "$recent"; then
  fail "Open Recent is empty after opening a file"
fi

python3 - "$shot" "$subshot" << 'PY'
import pathlib, sys
magic = b"\x89PNG\r\n\x1a\n"
for raw in sys.argv[1:]:
    path = pathlib.Path(raw)
    data = path.read_bytes()
    if not data.startswith(magic) or len(data) < 5000:
        raise SystemExit(f"{path} is not a real screenshot ({len(data)} bytes)")
    print(f"{path.name} {len(data)} bytes")
PY

echo "file menu captured"
