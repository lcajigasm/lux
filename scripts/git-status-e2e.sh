#!/usr/bin/env bash
# macOS: a real Lux.app draws the git status letter in the file tree.
# Git: Refresh Status must report note.txt:M, and the window is captured.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence/git-status"
mkdir -p "$out"
report="$out/git-status.txt"
: > "$report"

fail() {
  echo "FAIL $*" | tee -a "$report" >&2
  if [[ -s "${DRIVE_LOG:-}" ]]; then
    tail -40 "$DRIVE_LOG" >&2 || true
  fi
  exit 1
}

cat > /tmp/lux-window-id.swift << 'SWIFT'
import CoreGraphics
import Foundation
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 { continue }
    if owner.caseInsensitiveCompare("Lux") != .orderedSame && owner.caseInsensitiveCompare("lux") != .orderedSame {
        continue
    }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let id = w[kCGWindowNumber as String] as? Int ?? 0
    print("\(id)")
    exit(0)
}
exit(1)
SWIFT

shoot() {
  local dest="$1"
  local id
  id="$(swift /tmp/lux-window-id.swift)"
  screencapture -l "$id" -o "$dest"
  if [[ ! -s "$dest" ]]; then
    fail "screencapture produced no file at $dest"
  fi
}

app="$(bash "$root/scripts/stage-lux-app.sh")"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
drive_init "$app"
trap drive_stop EXIT

repo="$DRIVE_WORK/repo"
mkdir -p "$repo"
git init -b main "$repo"
git -C "$repo" config user.email "lux@example.com"
git -C "$repo" config user.name "Lux User"
printf 'alpha\n' > "$repo/note.txt"
git -C "$repo" add note.txt
git -C "$repo" commit -m "add note"
printf 'alpha\nbeta\n' > "$repo/note.txt"

drive_launch "$repo"
sleep 0.5
drive_key tap 35 cmd,shift
sleep 0.4
drive_key chars "refresh status"
sleep 0.4
drive_key tap 36 none

marks=""
for _ in $(seq 1 40); do
  drive_dump
  marks="$(drive_field git_marks)"
  if [[ "$marks" == *note.txt:M* ]]; then
    break
  fi
  sleep 0.3
done
if [[ "$marks" != *note.txt:M* ]]; then
  fail "refresh left git_marks=${marks:-missing}"
fi
sleep 0.4
shoot "$out/status-tree.png"
echo "PASS git_marks=$marks" | tee -a "$report"
