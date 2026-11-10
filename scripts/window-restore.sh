#!/usr/bin/env bash
# Open Lux, move and resize the window, quit, reopen, and compare the
# frame. Also checks that two tabs, the cursor, and an unsaved edit come
# back. The disconnected-monitor case is a unit test of restore_window;
# this runner has one display.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
report="$out/window-restore.txt"
: > "$report"

app="$(bash "$root/scripts/stage-lux-app.sh")"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
drive_init "$app"
trap drive_stop EXIT

pass() { echo "PASS $1" | tee -a "$report"; }
fail() { echo "FAIL $1" | tee -a "$report" >&2; exit 1; }

note() {
  printf '%s\n' "$*" | tee -a "$report"
}

a="$DRIVE_WORK/a.txt"
b="$DRIVE_WORK/b.txt"
printf 'alpha\n' > "$a"
printf 'beta\n' > "$b"

launch() {
  drive_stop
  rm -f "$DRIVE_PROBE"
  : > "$DRIVE_LOG"
  if [[ "$#" -eq 0 ]]; then
    "$DRIVE_APP/Contents/MacOS/lux" --probe "$DRIVE_PROBE" >"$DRIVE_LOG" 2>&1 &
  else
    "$DRIVE_APP/Contents/MacOS/lux" --probe "$DRIVE_PROBE" "$@" >"$DRIVE_LOG" 2>&1 &
  fi
  DRIVE_PID=$!
  drive_wait_window
  sleep 0.4
  drive_focus
}

frame() {
  swift /tmp/lux-find-window.swift "$DRIVE_OWNER"
}

quit_app() {
  local err="$DRIVE_WORK/quit.err"
  if ! osascript -e "tell application id \"dev.lux.editor\" to quit" >"$err" 2>&1; then
    echo "bundle quit failed:" >>"$report"
    cat "$err" >>"$report" || true
    osascript <<EOF >"$err" 2>&1 || true
tell application "System Events"
  tell (first process whose unix id is $DRIVE_PID)
    click menu item "Quit Lux" of menu "Lux" of menu bar 1
  end tell
end tell
EOF
    cat "$err" >>"$report" || true
  fi
  local _
  for _ in $(seq 1 40); do
    if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
      DRIVE_PID=""
      return 0
    fi
    sleep 0.25
  done
  echo "Lux did not quit. Menus:" >>"$report"
  osascript <<EOF >>"$report" 2>&1 || true
tell application "System Events"
  tell (first process whose unix id is $DRIVE_PID)
    get name of every menu of menu bar 1
  end tell
end tell
EOF
  return 1
}

abs_diff() {
  local left="$1" right="$2"
  if [[ "$left" -ge "$right" ]]; then
    echo $((left - right))
  else
    echo $((right - left))
  fi
}

launch "$a" "$b"
sleep 0.6
drive_dump
tabs="$(drive_field tabs)"
if [[ "$tabs" != "2" ]]; then
  fail "opened $tabs tabs, wanted 2"
fi
drive_key type "Z"
sleep 0.3
drive_dump
before_text="$(drive_field text)"
before_sel="$(drive_field sel)"
before_dirty="$(drive_field dirty)"
note "before text=$before_text sel=$before_sel dirty=$before_dirty tabs=$tabs"
if [[ "$before_text" != Z* ]]; then
  fail "typed text was $before_text"
fi
if [[ "$before_sel" == "0:0" ]]; then
  fail "cursor did not move ($before_sel)"
fi
if [[ "$before_dirty" != "1" ]]; then
  fail "buffer was not dirty"
fi
if grep -q Z "$a" || grep -q Z "$b"; then
  fail "the unsaved edit reached disk"
fi

osascript <<EOF
tell application "System Events"
  tell (first process whose unix id is $DRIVE_PID)
    set position of window 1 to {120, 80}
    set size of window 1 to {640, 480}
  end tell
end tell
EOF
sleep 0.6
before="$(frame)"
note "before frame $before"
read -r bx by bw bh <<<"$before"

if ! quit_app; then
  fail "quit did not exit"
fi
session="$HOME/.cache/lux/last-session.json"
if [[ ! -s "$session" ]]; then
  fail "no session at $session"
fi
note "session $session"

launch
sleep 0.8
after="$(frame)"
note "after frame $after"
read -r ax ay aw ah <<<"$after"
for pair in "$bx:$ax:x" "$by:$ay:y" "$bw:$aw:width" "$bh:$ah:height"; do
  left="${pair%%:*}"
  rest="${pair#*:}"
  right="${rest%%:*}"
  name="${rest#*:}"
  delta="$(abs_diff "$left" "$right")"
  if [[ "$delta" -gt 2 ]]; then
    fail "frame $name moved by ${delta}px ($left -> $right)"
  fi
done
pass "frame within 2px"

drive_dump
after_text="$(drive_field text)"
after_sel="$(drive_field sel)"
after_tabs="$(drive_field tabs)"
after_dirty="$(drive_field dirty)"
note "after text=$after_text sel=$after_sel dirty=$after_dirty tabs=$after_tabs"
if [[ "$after_tabs" != "2" ]]; then
  fail "restored $after_tabs tabs"
fi
if [[ "$after_text" != "$before_text" ]]; then
  fail "unsaved buffer came back as $after_text"
fi
if [[ "$after_sel" != "$before_sel" ]]; then
  fail "cursor came back as $after_sel"
fi
if [[ "$after_dirty" != "1" ]]; then
  fail "restored buffer was clean"
fi
if grep -q Z "$a" || grep -q Z "$b"; then
  fail "restore wrote the draft to disk"
fi
pass "tabs cursor and unsaved buffer"
note "RESULT pass"
