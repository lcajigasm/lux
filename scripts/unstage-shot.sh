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
# gix stores the workdir with symlinks resolved. The opened path has to
# use that same spelling or the hunk lookup misses the index entry.
repo="$(cd "$repo" && pwd -P)"

drive_launch "$repo/note.rs"
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$repo/note.rs" ]]; then
    break
  fi
  sleep 0.25
done
[[ "$(drive_field active_path)" == "$repo/note.rs" ]] || {
  echo "note.rs never became active (active=$(drive_field active_path))" >&2
  exit 1
}
# Command-Up is document start. The first staged hunk covers that line.
# The branch name arrives from the git worker; unstage before that still
# has the file path, but the caret has to be on the hunk.
drive_key tap 126 cmd
ready=0
for _ in $(seq 1 40); do
  drive_dump || true
  if [[ -n "$(drive_field branch)" && "$(drive_field sel)" == "0:0" ]]; then
    ready=1
    break
  fi
  sleep 0.25
done
[[ "$ready" == 1 ]] || {
  echo "caret or branch never settled (sel=$(drive_field sel) branch=$(drive_field branch))" >&2
  exit 1
}
opened=0
for _ in $(seq 1 4); do
  drive_key tap 35 cmd,shift
  sleep 0.4
  drive_dump || true
  if [[ "$(drive_field palette)" == "1" ]]; then
    opened=1
    break
  fi
done
[[ "$opened" == 1 ]] || {
  echo "palette did not open" >&2
  exit 1
}
drive_key chars "unstage hunk"
# Enter before the query lands runs the first command, which does not
# touch the index. Wait until the field shows the words that select
# Git: Unstage Hunk.
typed=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || true
  if [[ "$(drive_field palette_query)" == "unstage hunk" ]]; then
    typed=1
    break
  fi
done
[[ "$typed" == 1 ]] || {
  echo "palette query is '$(drive_field palette_query)', want unstage hunk" >&2
  exit 1
}
drive_key tap 36 ""

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
  drive_dump || true
  echo "palette=$(drive_field palette) query=$(drive_field palette_query) sel=$(drive_field sel) dirty=$(drive_field dirty) branch=$(drive_field branch)" >&2
  echo "announcement=$(drive_field announcement)" >&2
  tail -40 "${DRIVE_LOG:-}" >&2 || true
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
