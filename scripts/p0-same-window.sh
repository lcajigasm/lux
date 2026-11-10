#!/usr/bin/env bash
# One window: Command-P, Escape, Command-Shift-P. The palette must be open.
# A fresh window already passes Command-Shift-P. This is the chain that
# left palette=0 and goto=0 on macos-15.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
report="$out/same-window.txt"
: > "$report"

fail() {
  echo "FAIL $*" | tee -a "$report" >&2
  exit 1
}

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT

file="$DRIVE_WORK/chain.rs"
printf 'fn main() {}\n' > "$file"
drive_launch "$file"
sleep 0.4

drive_key tap 35 cmd
sleep 0.4
drive_dump
if [[ "$(drive_field goto)" != 1 ]]; then
  fail "Command-P left goto=$(drive_field goto) palette=$(drive_field palette)"
fi

drive_key tap 53 none
sleep 0.3
drive_key tap 35 cmd,shift
sleep 0.5
drive_dump
if [[ "$(drive_field palette)" != 1 ]]; then
  fail "Command-Shift-P left palette=$(drive_field palette) goto=$(drive_field goto)"
fi

echo "PASS same window ended with palette=1" | tee -a "$report"
echo "--> same-window palette done"
