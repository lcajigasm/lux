#!/usr/bin/env bash
# Build lux and capture its live window with docs/screenshots/example.rs open.
# macOS uses the standard desktop (screencapture). Linux uses Xvfb.
# Outputs PNGs in the given directory and copies them to the site twins.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

out="${1:-docs/screenshots}"
mkdir -p "$out"
sample="$root/docs/screenshots/example.rs"
bin="$root/target/debug/lux"
log="$out/lux.log"
pid=""

cleanup() {
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

if [[ ! -f "$sample" ]]; then
  echo "missing sample file: $sample" >&2
  exit 1
fi

echo "--> Building lux (debug, locked)"
cargo build -p lux --locked

keep_if_new() {
  local path="$1"
  local baseline="$2"
  if [[ ! -s "$path" ]]; then
    rm -f "$path"
    return 0
  fi
  local bytes
  bytes="$(wc -c < "$path" | tr -d ' ')"
  if [[ "$bytes" -lt 8000 ]]; then
    echo "--> Dropping $path ($bytes bytes, too small to be a window)"
    rm -f "$path"
    return 0
  fi
  if [[ -f "$baseline" ]] && cmp -s "$path" "$baseline"; then
    echo "--> Dropping $path (identical to $(basename "$baseline"))"
    rm -f "$path"
  fi
}

publish() {
  local name
  mkdir -p site/assets/screenshots docs/site/assets/screenshots
  for name in editor palette split; do
    if [[ -f "$out/$name.png" ]]; then
      cp "$out/$name.png" "site/assets/screenshots/$name.png"
      cp "$out/$name.png" "docs/site/assets/screenshots/$name.png"
      echo "--> $name.png"
    fi
  done
  if [[ ! -f "$out/editor.png" ]]; then
    echo "editor.png was not captured" >&2
    exit 1
  fi
}

if [[ "$(uname -s)" == "Darwin" ]]; then
  cat > /tmp/lux-window.swift << 'SWIFT'
import CoreGraphics
import Foundation

let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else {
    fputs("no window list\n", stderr)
    exit(1)
}
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let name = w[kCGWindowName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 { continue }
    if owner != "lux" && owner != "Lux" && name != "Lux" { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let id = (w[kCGWindowNumber as String] as? NSNumber)?.intValue ?? 0
    let x = (bounds["X"] as? NSNumber)?.intValue ?? 0
    let y = (bounds["Y"] as? NSNumber)?.intValue ?? 0
    print("\(id) \(x) \(y) \(Int(width)) \(Int(height))")
    exit(0)
}
exit(1)
SWIFT

  "$bin" "$sample" >"$log" 2>&1 &
  pid=$!
  echo "--> lux pid $pid"
  found=""
  for _ in $(seq 1 60); do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "lux exited before a window appeared" >&2
      cat "$log" >&2 || true
      exit 1
    fi
    if found="$(swift /tmp/lux-window.swift 2>/dev/null)"; then
      break
    fi
    found=""
    sleep 1
  done
  if [[ -z "$found" ]]; then
    echo "timed out waiting for the Lux window" >&2
    cat "$log" >&2 || true
    exit 1
  fi

  osascript -e 'tell application "System Events" to set frontmost of process "lux" to true' || true
  sleep 2

  shoot() {
    local dest="$1"
    local line id
    line="$(swift /tmp/lux-window.swift)"
    id="${line%% *}"
    screencapture -l "$id" -o "$dest"
  }

  shoot "$out/editor.png"
  keep_if_new "$out/editor.png" ""

  osascript << 'APPLESCRIPT' || true
tell application "System Events"
  set frontmost of process "lux" to true
  keystroke "p" using {command down, shift down}
end tell
APPLESCRIPT
  sleep 1
  shoot "$out/palette.png" || true
  keep_if_new "$out/palette.png" "$out/editor.png"

  osascript << 'APPLESCRIPT' || true
tell application "System Events"
  set frontmost of process "lux" to true
  key code 53
  keystroke "2" using {command down, option down}
end tell
APPLESCRIPT
  sleep 1
  shoot "$out/split.png" || true
  keep_if_new "$out/split.png" "$out/editor.png"
  if [[ -f "$out/split.png" && -f "$out/palette.png" ]]; then
    if cmp -s "$out/split.png" "$out/palette.png"; then
      rm -f "$out/split.png"
    fi
  fi

  publish
  exit 0
fi

if ! command -v xvfb-run >/dev/null || ! command -v import >/dev/null; then
  echo "Linux captures need xvfb-run and ImageMagick import" >&2
  exit 1
fi

export LIBGL_ALWAYS_SOFTWARE=1
export WGPU_BACKEND="${WGPU_BACKEND:-vulkan}"

xvfb-run -a -s '-screen 0 1280x800x24' bash -c '
  set -euo pipefail
  "$1" "$2" >"$3" 2>&1 &
  echo $! >"$4/lux.pid"
  sleep 8
  if ! kill -0 "$(cat "$4/lux.pid")" 2>/dev/null; then
    echo "lux exited before a window appeared" >&2
    cat "$3" >&2 || true
    exit 1
  fi
  import -window root "$4/editor.png"
' bash "$bin" "$sample" "$log" "$out"

keep_if_new "$out/editor.png" ""
publish
