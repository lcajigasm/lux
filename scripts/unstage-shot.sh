#!/usr/bin/env bash
# macos-15: unstage the hunk under the caret. The file on disk stays.
# The PNG is the compiled window after that command.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "unstage-shot runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

out="$root/docs/evidence"
mkdir -p "$out"
dest="$out/unstage-hunk.png"
rm -f "$dest"

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
printf 'one\ntwo\nthree\n' > "$repo/note.rs"
git -C "$repo" add note.rs
git -C "$repo" commit -m "base"
printf 'ONE\ntwo\nTHREE\n' > "$repo/note.rs"
git -C "$repo" add note.rs

drive_launch "$repo/note.rs"
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$repo/note.rs" ]]; then
    break
  fi
  sleep 0.25
done
[[ "$(drive_field active_path)" == "$repo/note.rs" ]] || {
  echo "note.rs never became active" >&2
  exit 1
}
drive_key tap 35 cmd,shift
sleep 0.35
drive_key chars "unstage hunk"
sleep 0.35
drive_key tap 36 none

unstaged=0
for _ in $(seq 1 40); do
  cached="$(git -C "$repo" diff --cached -- note.rs)"
  disk="$(cat "$repo/note.rs")"
  if [[ "$cached" != *ONE* && "$cached" == *THREE* && "$disk" == *ONE* ]]; then
    unstaged=1
    break
  fi
  sleep 0.25
done
[[ "$unstaged" == 1 ]] || {
  echo "index still has the first hunk or the file was rewritten" >&2
  git -C "$repo" diff --cached -- note.rs >&2 || true
  echo "disk=$(cat "$repo/note.rs")" >&2
  exit 1
}
sleep 0.3
id="$(swift /tmp/lux-window-id.swift)"
screencapture -l "$id" -o "$dest"
[[ -s "$dest" ]] || {
  echo "screencapture produced no unstage-hunk.png" >&2
  exit 1
}
echo "PASS $dest"
drive_stop
trap - EXIT
