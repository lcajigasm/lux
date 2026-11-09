#!/usr/bin/env bash
# Block 3 shortcut matrix on Lux.app. Each chord must change the file or
# the probe. US (ABC) is required. Spanish runs when the runner can select
# it; a missing layout is recorded and is not a pass.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
report="$out/shortcuts.txt"
: > "$report"

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT

select_layout() {
  local id="$1"
  swift -e "
import Carbon
import Foundation
let query = [kTISPropertyInputSourceID as String: \"$id\"] as CFDictionary
guard let list = TISCreateInputSourceList(query, true)?.takeRetainedValue() as? [TISInputSource],
      let source = list.first else { exit(2) }
_ = TISEnableInputSource(source)
if TISSelectInputSource(source) == noErr { print(\"$id\"); exit(0) }
exit(1)
" 2>/dev/null || true
}

pass() { echo "PASS $1" | tee -a "$report"; }
fail() { echo "FAIL $1: $2" | tee -a "$report" >&2; exit 1; }

field_is() {
  local key="$1" want="$2" got
  drive_dump
  got="$(drive_field "$key")"
  if [[ "$got" != "$want" ]]; then
    fail "$CASE" "$key is '$got', wanted '$want'"
  fi
}

file_is() {
  local want="$1" got
  drive_save
  got="$(cat "$CASE_FILE")"
  if [[ "$got" != "$want" ]]; then
    fail "$CASE" "file is $(printf %q "$got"), wanted $(printf %q "$want")"
  fi
}

begin() {
  CASE="$1"
  CASE_FILE="$DRIVE_WORK/$CASE.rs"
  printf '%s' "$2" > "$CASE_FILE"
  drive_launch "$CASE_FILE"
  sleep 0.4
}

run_matrix() {
  local layout="$1"
  echo "--> matrix on $layout" | tee -a "$report"

  begin palette ""
  drive_key tap 35 cmd,shift
  sleep 0.4
  field_is palette 1
  pass "$layout command_palette"

  begin quick ""
  drive_key tap 35 cmd
  sleep 0.4
  field_is goto 1
  field_is goto_mode anything
  pass "$layout quick_open"

  begin next "one one"$'\n'
  drive_key tap 2 cmd
  sleep 0.2
  drive_key type Z
  sleep 0.2
  file_is "Z one"$'\n'
  pass "$layout select_next_occurrence"

  begin skip "one one"$'\n'
  drive_key tap 2 cmd
  sleep 0.2
  drive_key tap 40 cmd
  sleep 0.15
  drive_key tap 2 cmd
  sleep 0.2
  drive_key type Z
  sleep 0.2
  file_is "one Z"$'\n'
  pass "$layout skip_next_occurrence"

  begin split $'ab\ncd\n'
  drive_key tap 0 cmd
  sleep 0.2
  drive_key tap 37 cmd,shift
  sleep 0.3
  drive_dump
  local sel
  sel="$(drive_field sel)"
  if [[ "$sel" != *,* ]]; then
    fail "$CASE" "split did not make two selections ($sel)"
  fi
  pass "$layout split_selection_into_lines"

  begin comment $'fn a() {}\n'
  drive_key tap 44 cmd
  sleep 0.3
  drive_save
  if ! grep -q '//' "$CASE_FILE"; then
    fail "$CASE" "file was not commented: $(cat "$CASE_FILE")"
  fi
  pass "$layout toggle_comment"

  begin below $'a\nb\n'
  drive_key tap 125 ctrl,shift
  sleep 0.2
  drive_key type Z
  sleep 0.2
  file_is $'Za\nZb\n'
  pass "$layout add_cursor_below"

  begin above $'a\nb\n'
  drive_key tap 125 none
  sleep 0.2
  drive_key tap 126 ctrl,shift
  sleep 0.2
  drive_key type Z
  sleep 0.2
  file_is $'Za\nZb\n'
  pass "$layout add_cursor_above"

  begin find $'aa xx aa\n'
  drive_key tap 3 cmd
  sleep 0.3
  field_is find 1
  pass "$layout find"

  begin replace ""
  drive_key tap 3 cmd,alt
  sleep 0.3
  field_is replace 1
  pass "$layout replace"

  begin files ""
  drive_key tap 3 cmd,shift
  sleep 0.4
  field_is project_search 1
  pass "$layout find_in_files"

  begin nextprev $'aa xx aa\n'
  drive_key tap 3 cmd
  sleep 0.2
  drive_key chars aa
  sleep 0.3
  drive_key tap 5 cmd
  sleep 0.3
  drive_dump
  local head
  head="$(drive_field sel | awk -F'[:,]' '{print $2}')"
  if [[ -z "$head" || "$head" -eq 0 ]]; then
    fail "$CASE" "find_next left the caret at '$head'"
  fi
  drive_key tap 5 cmd,shift
  sleep 0.3
  drive_dump
  head="$(drive_field sel | awk -F'[:,]' '{print $2}')"
  if [[ "$head" != 0 ]]; then
    fail "$CASE" "find_prev left the caret at '$head'"
  fi
  pass "$layout find_next_prev"

  begin symbol $'\nfn sample_symbol() {}\n'
  drive_key tap 15 cmd
  sleep 0.3
  field_is goto 1
  field_is goto_query @
  pass "$layout goto_symbol"

  begin project $'\nfn sample_symbol() {}\n'
  drive_key tap 15 cmd,shift
  sleep 0.4
  field_is goto_mode project
  pass "$layout goto_symbol_in_project"

  begin save ""
  drive_key type "ShortcutSave"
  sleep 0.2
  file_is "ShortcutSave"
  pass "$layout save"

  begin undo ""
  drive_key type Q
  sleep 0.2
  drive_key tap 6 cmd
  sleep 0.3
  drive_dump
  if [[ "$(drive_field text)" != "" ]]; then
    fail "$CASE" "undo left '$(drive_field text)'"
  fi
  drive_key tap 6 cmd,shift
  sleep 0.3
  drive_dump
  if [[ "$(drive_field text)" != "Q" ]]; then
    fail "$CASE" "redo left '$(drive_field text)'"
  fi
  pass "$layout undo_redo"

  begin columns ""
  drive_key tap 19 cmd,alt
  sleep 0.4
  field_is panes 2
  pass "$layout set_layout_columns_2"

  begin line $'hello\n'
  drive_key tap 37 cmd
  sleep 0.2
  drive_key type Z
  sleep 0.2
  drive_save
  if grep -q hello "$CASE_FILE"; then
    fail "$CASE" "expand_to_line left $(cat "$CASE_FILE")"
  fi
  pass "$layout expand_to_line"

  begin dup $'hello\n'
  drive_key tap 2 cmd,shift
  sleep 0.3
  drive_save
  local n
  n="$(grep -c hello "$CASE_FILE")"
  if [[ "$n" -lt 2 ]]; then
    fail "$CASE" "duplicate left $(cat "$CASE_FILE")"
  fi
  pass "$layout duplicate_line"

  begin delete $'hello\nworld\n'
  drive_key tap 40 cmd,shift
  sleep 0.3
  drive_save
  if grep -q hello "$CASE_FILE"; then
    fail "$CASE" "delete_line left $(cat "$CASE_FILE")"
  fi
  if ! grep -q world "$CASE_FILE"; then
    fail "$CASE" "delete_line removed world: $(cat "$CASE_FILE")"
  fi
  pass "$layout delete_line"

  drive_stop
}

echo "--> selecting ABC"
if [[ "$(select_layout com.apple.keylayout.ABC)" == com.apple.keylayout.ABC ]]; then
  echo "layout ABC" | tee -a "$report"
else
  echo "layout ABC not selectable; running on the runner default" | tee -a "$report"
fi
run_matrix us

echo "--> selecting Spanish"
spanish="$(select_layout com.apple.keylayout.Spanish-ISO)"
if [[ "$spanish" != com.apple.keylayout.Spanish-ISO ]]; then
  spanish="$(select_layout com.apple.keylayout.Spanish)"
fi
if [[ "$spanish" == com.apple.keylayout.Spanish* ]]; then
  echo "layout $spanish" | tee -a "$report"
  run_matrix es
else
  echo "RESULT layout-unavailable" | tee -a "$report"
  echo "Spanish layout could not be selected. The US matrix above is not a Spanish pass."
fi
echo "--> shortcut matrix done"
