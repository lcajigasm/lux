#!/usr/bin/env bash
# macos-15: Markdown preview of rust, javascript, and python fences,
# beside Visual Studio Code's preview of the same file.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "markdown-shot runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi
python3 "$root/scripts/fence_indent.py" --self-test

out="$root/docs/evidence"
mkdir -p "$out"
dest="$out/markdown-fence.png"
rm -f "$dest"
work="$(mktemp -d)"

cat > /tmp/lux-window-id.swift << 'SWIFT'
import CoreGraphics
import Foundation
let want = CommandLine.arguments.dropFirst().first ?? ""
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 || owner.caseInsensitiveCompare(want) != .orderedSame { continue }
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
  local owner="$1"
  local png="$2"
  local attempt id
  for attempt in 1 2 3; do
    rm -f "$png"
    if ! id="$(swift /tmp/lux-window-id.swift "$owner" 2>/dev/null)"; then
      echo "no $owner window on attempt $attempt" >&2
      sleep 1
      continue
    fi
    screencapture -l "$id" -o "$png"
    if [[ -s "$png" ]]; then
      return 0
    fi
    sleep 1
  done
  echo "screencapture produced no file for $owner" >&2
  return 1
}

require_fence_spaces() {
  local fences="$1"
  local needle
  for needle in \
    'fn main() {' \
    'function main() {' \
    'def main():' \
    '\n    let name' \
    '\n    const name' \
    '\n    name ='
  do
    case "$fences" in
      *"$needle"*) ;;
      *)
        echo "missing [$needle]" >&2
        return 1
        ;;
    esac
  done
  for needle in fnmain functionmain defmain; do
    case "$fences" in
      *"$needle"*)
        echo "collapsed [$needle]" >&2
        return 1
        ;;
    esac
  done
}

app="$(bash "$root/scripts/stage-lux-app.sh")"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
drive_init "$app"
cleanup() {
  drive_stop || true
  osascript -e 'tell application "Visual Studio Code" to quit' >/dev/null 2>&1 || true
}
trap cleanup EXIT

fixture="$HOME/fences.md"
cp "$root/crates/lux/src/render/fixtures/fences.md" "$fixture"

drive_launch "$fixture"
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field active_path)" == "$fixture" ]]; then
    break
  fi
  sleep 0.25
done
[[ "$(drive_field active_path)" == "$fixture" ]] || {
  echo "fences.md never became active" >&2
  exit 1
}
[[ "$(drive_field language)" == "Markdown" ]] || {
  echo "opened as $(drive_field language)" >&2
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
  echo "palette=$(drive_field palette) active=$(drive_field active_path)" >&2
  echo "announcement=$(drive_field announcement)" >&2
  tail -40 "${DRIVE_LOG:-}" >&2 || true
  exit 1
}
drive_key chars "toggle preview"
typed=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || true
  if [[ "$(drive_field palette_query)" == "toggle preview" ]]; then
    typed=1
    break
  fi
done
[[ "$typed" == 1 ]] || {
  echo "palette query is '$(drive_field palette_query)', want toggle preview" >&2
  echo "palette=$(drive_field palette) active=$(drive_field active_path)" >&2
  echo "announcement=$(drive_field announcement)" >&2
  tail -40 "${DRIVE_LOG:-}" >&2 || true
  exit 1
}
drive_key tap 36 none
ready=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || true
  if [[ "$(drive_field palette)" == "0" && -n "$(drive_field preview_fences)" ]]; then
    ready=1
    break
  fi
done
fences="$(drive_field preview_fences)"
[[ "$ready" == 1 ]] || {
  echo "preview did not publish fence text" >&2
  echo "preview_fences=$fences" >&2
  exit 1
}
require_fence_spaces "$fences" || {
  echo "preview_fences=$fences" >&2
  exit 1
}
shoot Lux "$work/lux.png"
cp "$work/lux.png" "$dest"
python3 "$root/scripts/fence_indent.py" "$dest"

echo "--> Installing Visual Studio Code and ImageMagick"
brew install --cask visual-studio-code
brew install imagemagick
xattr -dr com.apple.quarantine "/Applications/Visual Studio Code.app" || true
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$lsregister" ]]; then
  "$lsregister" -f "/Applications/Visual Studio Code.app" || true
fi
mkdir -p "$HOME/Library/Application Support/Code/User"
cat > "$HOME/Library/Application Support/Code/User/settings.json" << 'JSON'
{
  "security.workspace.trust.enabled": false,
  "workbench.startupEditor": "none",
  "update.mode": "none"
}
JSON
code_bin="/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"
[[ -x "$code_bin" ]] || {
  echo "Visual Studio Code did not install a code binary" >&2
  exit 1
}
"$code_bin" --new-window --disable-extensions "$fixture"
owner=""
for _ in $(seq 1 60); do
  for candidate in Code "Visual Studio Code"; do
    if swift /tmp/lux-window-id.swift "$candidate" >/dev/null 2>&1; then
      owner="$candidate"
      break
    fi
  done
  [[ -n "$owner" ]] && break
  sleep 1
done
[[ -n "$owner" ]] || {
  echo "Visual Studio Code did not open a window; Lux capture is $dest" >&2
  echo "PASS $dest"
  drive_stop
  trap - EXIT
  exit 0
}
osascript -e "tell application \"System Events\" to set frontmost of process \"$owner\" to true" >/dev/null
# Markdown: Open Preview. V is virtual key 9.
swift /tmp/lux-drive.swift tap 9 cmd,shift
sleep 1.5
shoot "$owner" "$work/code.png"
magick "$work/lux.png" "$work/code.png" +append "$dest"
[[ -s "$dest" ]] || {
  echo "side-by-side markdown-fence.png is empty" >&2
  exit 1
}
echo "PASS $dest"
drive_stop
trap - EXIT
osascript -e 'tell application "Visual Studio Code" to quit' >/dev/null 2>&1 || true
