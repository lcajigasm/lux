#!/usr/bin/env bash
# macOS Lux.app. One git project on main, nested folders, ci.yml open
# from .github/workflows, and an Untitled tab. The × on that project's
# header must close the project and leave the window up.
# Closing the last folder used to drop the sidebar while focus stayed on
# the file tree. The next accessibility update panics:
# "Focused ID ... is not in the node list".
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "close-root captures run on macOS. This host is $(uname -s)." >&2
  exit 1
fi

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
export RUST_BACKTRACE=1

out="$root/docs/evidence/close-root"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/*.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

repo="$HOME/MAC-RESTORE"
mkdir -p "$repo/.claude" "$repo/.github/workflows" "$repo/scripts" "$repo/tests"
printf 'name: ci\n' > "$repo/.github/workflows/ci.yml"
printf 'notes\n' > "$repo/.claude/notes.md"
printf 'echo ok\n' > "$repo/scripts/run.sh"
printf 'fn main() {}\n' > "$repo/tests/app.rs"
git -C "$repo" init -b main
git -C "$repo" config user.email "lux@example.com"
git -C "$repo" config user.name "Lux User"
git -C "$repo" add .claude .github scripts tests
git -C "$repo" commit -m "add project"

fail() {
  echo "FAIL $*" | tee -a "$out/close-root.txt" >&2
  if [[ -s "${DRIVE_LOG:-}" ]]; then
    cp "$DRIVE_LOG" "$out/lux.log"
    tail -80 "$DRIVE_LOG" >&2 || true
  fi
  exit 1
}

drive_init "$root/target/Lux.app"
trap drive_stop EXIT

drive_launch "$repo/.github/workflows/ci.yml" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/frame.png" \
  "$repo"

ready=0
for _ in $(seq 1 40); do
  if drive_dump \
    && [[ "$(drive_field roots)" == *MAC-RESTORE* ]] \
    && [[ "$(drive_field active_path)" == *MAC-RESTORE/.github/workflows/ci.yml ]] \
    && [[ "$(drive_field tree)" == *workflows* ]] \
    && [[ "$(drive_field branch)" == "main" ]]; then
    ready=1
    break
  fi
  sleep 0.3
done
if [[ "$ready" != 1 ]]; then
  fail "project was not ready: roots=$(drive_field roots) path=$(drive_field active_path) branch=$(drive_field branch) tree=$(drive_field tree)"
fi

# An Untitled tab beside the open workflow file. Cmd+N activates it;
# the first tab is ci.yml and has to be active again, matching the report.
drive_key tap 45 cmd
sleep 0.3
drive_dump || fail "Lux exited after New"
content="$(drive_field content)"
sidebar_w="$(drive_field sidebar_w)"
scale="$(drive_field scale)"
if [[ -z "$content" || -z "$sidebar_w" || -z "$scale" ]]; then
  fail "probe did not report the content rect"
fi
IFS=',' read -r cx cy _cw _ch <<<"$content"
# Tabs start at the sidebar's right edge. The first one is ci.yml.
tab_x="$(python3 -c "print(int(float('$cx') + float('$sidebar_w') + 24 * float('$scale')))")"
tab_y="$(python3 -c "print(int(float('$cy') + 16 * float('$scale')))")"
drive_click "$tab_x" "$tab_y"
sleep 0.3
active=""
tabs=0
for _ in $(seq 1 20); do
  drive_dump || fail "Lux exited before the header was clicked"
  active="$(drive_field active_path)"
  tabs="$(drive_field tabs)"
  if [[ "$tabs" -ge 2 && "$active" == *ci.yml ]]; then
    break
  fi
  sleep 0.2
done
if [[ "$tabs" -lt 2 || "$active" != *ci.yml ]]; then
  fail "wanted ci.yml plus Untitled, got tabs=$tabs active=$active"
fi

rm -f "$out/frame.png"
kill -USR1 "$DRIVE_PID"
for _ in $(seq 1 40); do
  if [[ -s "$out/frame.png" ]]; then
    break
  fi
  sleep 0.25
done
if [[ ! -s "$out/frame.png" ]]; then
  fail "before snapshot missing"
fi
cp "$out/frame.png" "$out/before.png"

# Ask AppKit for the window. That activates the accessibility tree, which
# is what turns the stale file-tree focus into the panic.
cat > /tmp/lux-ax-window.swift << 'SWIFT'
import ApplicationServices
import Foundation
let pid = pid_t(CommandLine.arguments[1])!
let app = AXUIElementCreateApplication(pid)
var value: CFTypeRef?
let err = AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value)
fputs("ax windows \(err.rawValue)\n", stderr)
SWIFT
swift /tmp/lux-ax-window.swift "$DRIVE_PID" || true
sleep 0.4

drive_dump || fail "Lux exited when accessibility was queried"
sidebar_w="$(drive_field sidebar_w)"
scale="$(drive_field scale)"
content="$(drive_field content)"
IFS=',' read -r cx cy _cw _ch <<<"$content"
read -r click_x click_y < <(python3 -c "
cx=float('$cx'); cy=float('$cy'); scale=float('$scale'); sidebar=float('$sidebar_w')
print(int(cx + sidebar - 14 * scale), int(cy + (32 + 14) * scale))
")
echo "header close at $click_x $click_y sidebar=$sidebar_w scale=$scale" | tee -a "$out/close-root.txt"
drive_click "$click_x" "$click_y"
sleep 0.6

if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
  fail "Lux exited when the project header was closed"
fi
if ! drive_dump; then
  fail "Lux exited when the project header was closed"
fi

roots="$(drive_field roots)"
tree="$(drive_field tree)"
sidebar="$(drive_field sidebar)"
branch="$(drive_field branch)"
tab_paths="$(drive_field tab_paths)"
active="$(drive_field active_path)"
if [[ "$roots" == *MAC-RESTORE* || "$tree" == *workflows* || "$sidebar" != "0" ]]; then
  fail "project stayed open: roots=$roots sidebar=$sidebar tree=$tree"
fi
if [[ "$tab_paths" == *ci.yml* || -n "$active" ]]; then
  fail "ci.yml stayed open: tabs=$tab_paths active=$active"
fi
if [[ "$branch" == "main" ]]; then
  fail "git branch stayed on the closed project"
fi
if [[ ! -f "$repo/.github/workflows/ci.yml" ]]; then
  fail "closing the project removed ci.yml from disk"
fi

rm -f "$out/frame.png"
kill -USR1 "$DRIVE_PID"
for _ in $(seq 1 40); do
  if [[ -s "$out/frame.png" ]]; then
    break
  fi
  sleep 0.25
done
if [[ ! -s "$out/frame.png" ]]; then
  fail "after snapshot missing"
fi
cp "$out/frame.png" "$out/after.png"

{
  echo "close root"
  echo "roots=$roots"
  echo "sidebar=$sidebar"
  echo "branch=$branch"
  echo "tabs=$tab_paths"
  echo "active=$active"
  echo "ci.yml still on disk"
  ls -1 "$out"/*.png
} | tee -a "$out/close-root.txt"
