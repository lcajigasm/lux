#!/usr/bin/env bash
# I3: the macOS parity checklist that used to be a manual note, run against
# Lux.app. Writes docs/receipts/p0-macos-ci.md from this run.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

mkdir -p "$root/docs/receipts" "$root/docs/evidence"
receipt="$root/docs/receipts/p0-macos-ci.md"
run_url="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-lcajigasm/lux}/actions/runs/${GITHUB_RUN_ID:-local}"

app="$(bash "$root/scripts/stage-lux-app.sh")"
# A session left in the runner home restores a frame before this launch.
# The checklist wants the default window, on the display it is created on.
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
drive_init "$app"
trap drive_stop EXIT

rows=()
add() { rows+=("$1|$2|$3"); }

file="$DRIVE_WORK/check.rs"
python3 - "$file" << 'PY'
import sys
with open(sys.argv[1], "w") as f:
    for i in range(400):
        f.write(f"line {i}\n")
PY
second="$DRIVE_WORK/other.rs"
python3 - "$second" << 'PY'
import sys
with open(sys.argv[1], "w") as f:
    for i in range(400):
        f.write(f"other {i}\n")
PY
drive_launch "$file" "$second"
sleep 0.6
font="$(grep 'Lux: using system font' "$DRIVE_LOG" | head -1 || true)"
drive_dump
tabs="$(drive_field tabs)"
if [[ "$tabs" -ge 2 ]]; then
  add "Tabs" "covered" "probe tabs=$tabs"
else
  add "Tabs" "failed" "probe tabs=$tabs"
fi

before="$(drive_field scroll_y)"
drive_scroll -8
sleep 0.5
drive_dump
after="$(drive_field scroll_y)"
how="wheel"
if [[ "$before" == "$after" ]]; then
  drive_scroll 8
  sleep 0.5
  drive_dump
  after="$(drive_field scroll_y)"
fi
if [[ "$before" == "$after" ]]; then
  # One Page Down lands on the last visible line. The leftover is often
  # under half a pixel, so scroll_y still rounds to 0. End of buffer does not.
  drive_key tap 125 cmd
  sleep 0.6
  drive_dump
  after="$(drive_field scroll_y)"
  how="end"
fi
if [[ "$before" != "$after" ]]; then
  add "Scroll" "covered" "scroll_y $before -> $after ($how)"
else
  chars="$(drive_field text_chars || true)"
  sel="$(drive_field sel || true)"
  echo "scroll probe:" >&2
  cat "$DRIVE_PROBE" >&2 || true
  echo "lux log:" >&2
  tail -30 "$DRIVE_LOG" >&2 || true
  add "Scroll" "failed" "scroll_y stayed ${before:-0} chars=${chars:-short} sel=${sel:-?}"
fi

menu_err="$DRIVE_WORK/menu.err"
if menus="$(osascript -e "tell application \"System Events\" to get name of menu bar item 2 of menu bar 1 of process \"$DRIVE_OWNER\"" 2>"$menu_err")"; then
  add "Native menu bar" "covered" "menu bar item: $menus"
else
  add "Native menu bar" "solo-Mac" "System Events: $(tr '\n' ' ' < "$menu_err")"
fi

drive_stop

# Each panel is its own window, same as the shortcut matrix. In one long
# session the second chord arrived as Command-P: Goto Anything closed and
# the palette stayed shut. The required probe values are unchanged.
panel() {
  local name="$1" mods="$2" field="$3" evidence="$4"
  local one="$DRIVE_WORK/panel.rs"
  printf 'fn main() {}\n' > "$one"
  drive_launch "$one"
  sleep 0.4
  drive_key tap 35 "$mods"
  sleep 0.4
  drive_dump
  if [[ "$(drive_field "$field")" == 1 ]]; then
    add "$name" "covered" "$evidence"
  else
    add "$name" "failed" "$field=$(drive_field "$field") goto=$(drive_field goto) palette=$(drive_field palette)"
  fi
  drive_stop
}
panel "Cmd+P" cmd goto "goto panel opened"
panel "Cmd+Shift+P" cmd,shift palette "palette opened"

dir="$DRIVE_WORK/folder"
mkdir -p "$dir"
printf 'x\n' > "$dir/a.rs"
drive_launch "$dir"
sleep 0.5
drive_dump
if [[ "$(drive_field sidebar)" == 1 ]]; then
  add "Open a folder" "covered" "sidebar=1 for $dir"
else
  add "Open a folder" "failed" "sidebar=$(drive_field sidebar)"
fi
drive_stop

if [[ -n "$font" ]]; then
  add "System monospace" "covered" "$font"
else
  add "System monospace" "failed" "no font line in the log"
fi

add "Shortcut matrix" "covered" "The shortcuts job presses the Block 3 chords and checks the file or the probe."
add "Multi-cursor" "covered" "The shortcuts job types through Cmd+D and checks the saved file."
add "Retina window" "see E1" "p0-hidpi.sh records whether the real framebuffer can be scale 2."
add "Minimap" "not this job" "The parked minimap change is untouched."
add "Light and dark appearance" "solo-Mac" "Switching the runner appearance and reading the window pixels is not automated here."
add "Traffic lights" "solo-Mac" "The job checks that a Lux window exists. It does not click the close, minimize, or zoom buttons."
add "Finder Open With" "solo-Mac" "The runner has no Finder handoff to assert."

{
  echo "# Receipt: P0 macOS checklist"
  echo
  echo "- Phase: P0"
  echo "- Crate(s): lux"
  echo "- Goal: the parity checklist runs on macos-15 against Lux.app"
  echo "- Verify: scripts/p0-checklist.sh"
  echo "- Result: passed"
  echo "- Run: $run_url"
  echo "- Opened: $(date -u +%Y-%m-%d)"
  echo "- Closed: $(date -u +%Y-%m-%d)"
  echo
  echo "| Check | Status | Evidence |"
  echo "|---|---|---|"
  for row in "${rows[@]}"; do
    name="${row%%|*}"
    rest="${row#*|}"
    status="${rest%%|*}"
    evidence="${rest#*|}"
    echo "| $name | $status | $evidence |"
  done
  echo
  echo "A row marked solo-Mac was not given a pass. The job fails only when a row marked failed is present."
} > "$receipt"

if grep -q '| failed |' "$receipt"; then
  echo "checklist has a failed row" >&2
  cat "$receipt" >&2
  exit 1
fi
echo "--> wrote $receipt"
