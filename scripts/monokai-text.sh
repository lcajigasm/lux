#!/usr/bin/env bash
# Real Lux window on macos-15. Scale 1 used to paint an LCD fringe
# beside neutral stems. The capture is the window itself, then an 8×
# nearest-neighbor crop of a stem. A second dark theme and a light theme
# take the same path.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "monokai text captures run on macOS. This host is $(uname -s)." >&2
  exit 1
fi

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
mkdir -p "$HOME/.config/lux"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/monokai-text"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/*.txt

HOME="$real_home" bash "$root/scripts/stage-lux-app.sh"
drive_init "$root/target/Lux.app"

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
    let id = (w[kCGWindowNumber as String] as? NSNumber)?.intValue ?? 0
    print(id)
    exit(0)
}
exit(1)
SWIFT

fixture="$HOME/monokai-sample.rs"
cat > "$fixture" << 'EOF'
fn greet(name: &str) -> String {
    // MMMM comment stays with the comment color.
    let MMMM = 1;
    format!("MMMM {name}")
}
EOF

write_theme() {
  local name="$1"
  python3 - "$HOME/.config/lux/settings.json" "$name" << 'PY'
import json, sys
path, name = sys.argv[1], sys.argv[2]
json.dump({
    "editor.fontSize": 16.0,
    "editor.lineHeight": 1.4,
    "workbench.colorTheme": name,
}, open(path, "w"), indent=2)
PY
}

shoot() {
  local dest="$1"
  local id
  id="$(swift /tmp/lux-window-id.swift "$DRIVE_OWNER")"
  screencapture -l "$id" -o "$dest"
  if [[ ! -s "$dest" ]]; then
    echo "screencapture produced no file at $dest" >&2
    exit 1
  fi
}

capture_theme() {
  local name="$1"
  local slug="$2"
  local scheme="$3"
  write_theme "$name"
  drive_launch "$fixture"
  sleep 0.6
  shoot "$out/$slug.png"
  local bg fg
  bg="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["globals"]["background"])' "$scheme")"
  fg="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["globals"]["foreground"])' "$scheme")"
  python3 "$root/scripts/monokai_text_pixels.py" "$out/$slug.png" "$bg" "$fg" "$out/$slug-zoom.png"
}

capture_theme "monokai" "monokai" "$root/themes/Monokai.sublime-color-scheme"
capture_theme "dracula" "dracula" "$root/themes/Dracula.sublime-color-scheme"
capture_theme "solarized light" "solarized-light" "$root/themes/Solarized Light.sublime-color-scheme"
drive_stop

{
  echo "Lux captures are screencapture of the live window."
  echo "VS Code: attempting a real window. A missing file means it did not appear."
} > "$out/vscode.txt"

if ! command -v code >/dev/null 2>&1; then
  brew install --cask visual-studio-code >>"$out/vscode.txt" 2>&1 || {
    echo "brew install --cask visual-studio-code failed" | tee -a "$out/vscode.txt"
  }
fi

if command -v code >/dev/null 2>&1; then
  data="$out/vscode-data"
  mkdir -p "$data/User"
  printf '%s\n' '{ "workbench.colorTheme": "Monokai" }' > "$data/User/settings.json"
  code --user-data-dir "$data" --new-window "$fixture" >>"$out/vscode.txt" 2>&1 || true
  appeared=0
  for _ in $(seq 1 30); do
    if swift /tmp/lux-window-id.swift "Code" >/dev/null 2>&1; then
      appeared=1
      break
    fi
    sleep 1
  done
  if [[ "$appeared" == 1 ]]; then
    id="$(swift /tmp/lux-window-id.swift Code)"
    screencapture -l "$id" -o "$out/vscode-monokai.png" || true
    if [[ -s "$out/vscode-monokai.png" ]]; then
      echo "captured $out/vscode-monokai.png" | tee -a "$out/vscode.txt"
      python3 "$root/scripts/monokai_text_pixels.py" \
        "$out/vscode-monokai.png" "#272822" "#F8F8F2" "$out/vscode-monokai-zoom.png" \
        >>"$out/vscode.txt" 2>&1 || echo "VS Code pixel check did not pass; the PNG is still the real window" | tee -a "$out/vscode.txt"
    fi
  else
    echo "VS Code window did not appear" | tee -a "$out/vscode.txt"
  fi
else
  echo "code is not on PATH" | tee -a "$out/vscode.txt"
fi
