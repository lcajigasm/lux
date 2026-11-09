#!/usr/bin/env bash
# C2 autopair and C4 pause/word undo, typed into Lux.app. The saved file
# and the probe selection are the assertions.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
report="$out/editing.txt"
: > "$report"

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT

pass() { echo "PASS $1" | tee -a "$report"; }
fail() { echo "FAIL $1" | tee -a "$report" >&2; exit 1; }

fresh() {
  CASE_FILE="$DRIVE_WORK/edit.rs"
  printf '' > "$CASE_FILE"
  drive_launch "$CASE_FILE"
  sleep 0.4
}

saved() {
  drive_save
  cat "$CASE_FILE"
}

pair() {
  local open="$1" close="$2" name="$3"
  fresh
  drive_key type "$open"
  sleep 0.2
  local got
  got="$(saved)"
  if [[ "$got" != "${open}${close}" ]]; then
    fail "pair $name produced $(printf %q "$got")"
  fi
  fresh
  drive_key type "$open"
  sleep 0.15
  drive_key type "$close"
  sleep 0.15
  got="$(saved)"
  if [[ "$got" != "${open}${close}" ]]; then
    fail "skip $name produced $(printf %q "$got")"
  fi
  fresh
  drive_key type "$open"
  sleep 0.15
  drive_key tap 51 none
  sleep 0.15
  got="$(saved)"
  if [[ -n "$got" ]]; then
    fail "delete $name produced $(printf %q "$got")"
  fi
  pass "autopair $name"
}

pair '(' ')' '()'
pair '[' ']' '[]'
pair '{' '}' '{}'
pair '"' '"' 'quotes'
pair "'" "'" "single-quotes"

fresh
drive_key chars ab
sleep 1.2
drive_key chars c
sleep 0.2
drive_key tap 6 cmd
sleep 0.4
drive_dump
text="$(drive_field text)"
sel="$(drive_field sel)"
if [[ "$text" != "ab" ]]; then
  fail "pause undo left $(printf %q "$text")"
fi
if [[ "$sel" != "2:2" ]]; then
  fail "pause undo selection is $sel"
fi
pass "undo pause"

fresh
drive_key chars "hello world"
sleep 0.2
drive_key tap 6 cmd
sleep 0.4
drive_dump
text="$(drive_field text)"
sel="$(drive_field sel)"
if [[ "$text" != "hello " ]]; then
  fail "word undo left $(printf %q "$text")"
fi
if [[ "$sel" != "6:6" ]]; then
  fail "word undo selection is $sel"
fi
pass "undo word"
echo "--> editing checks done"
