#!/usr/bin/env bash
# Real Lux window on macos-15. Scale 1 used to paint an LCD fringe
# beside neutral stems. The capture is the window itself, then an 8×
# nearest-neighbor crop of a stem. Every bundled scheme takes the same
# path.
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
  write_theme "$name"
  drive_launch "$fixture"
  sleep 0.6
  shoot "$out/$slug.png"
}

count=0
failed=0
results="$out/schemes.txt"
: > "$results"
for scheme in "$root"/themes/*.sublime-color-scheme; do
  base="$(basename "$scheme" .sublime-color-scheme)"
  name="$(printf '%s' "$base" | tr '[:upper:]' '[:lower:]')"
  slug="$(printf '%s' "$name" | tr ' ' '-')"
  capture_theme "$name" "$slug"
  bg="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["globals"]["background"])' "$scheme")"
  fg="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["globals"]["foreground"])' "$scheme")"
  # One row per scheme. A miss does not stop the loop.
  line="$(python3 "$root/scripts/monokai_text_pixels.py" "$out/$slug.png" "$bg" "$fg" "$out/$slug-zoom.png" 2>&1)" || true
  printf '%s\n' "$line" | tee -a "$results"
  case "$line" in
    PASS*) ;;
    *) failed=1 ;;
  esac
  count=$((count + 1))
done
[[ "$count" -ge 12 ]] || {
  echo "expected every bundled scheme, found $count" >&2
  exit 1
}
echo "---- schemes ----"
cat "$results"
drive_stop

# Lux is done. Sublime and VS Code use the runner account, not the
# temporary Lux home.
export HOME="$real_home"

sublime_compare() {
  local note="$out/sublime.txt"
  echo "Sublime Text: real window of the same fixture. No PNG means the window did not appear." | tee "$note"
  export HOMEBREW_NO_AUTO_UPDATE=1
  export NONINTERACTIVE=1
  local app="/Applications/Sublime Text.app"
  if [[ ! -d "$app" ]]; then
    brew install --cask sublime-text >>"$note" 2>&1 || {
      echo "Sublime Text window did not appear: brew install --cask sublime-text failed" | tee -a "$note"
      return 0
    }
  fi
  if ! command -v magick >/dev/null 2>&1; then
    brew install imagemagick >>"$note" 2>&1 || {
      echo "imagemagick is not available; the side-by-side will not be written" | tee -a "$note"
    }
  fi
  local subl="$app/Contents/SharedSupport/bin/subl"
  if [[ ! -x "$subl" ]]; then
    echo "Sublime Text window did not appear: no subl binary at $subl" | tee -a "$note"
    return 0
  fi
  xattr -dr com.apple.quarantine "$app" || true
  local pref_dir="$HOME/Library/Application Support/Sublime Text/Packages/User"
  mkdir -p "$pref_dir"
  cat > "$pref_dir/Preferences.sublime-settings" << 'JSON'
{
  "color_scheme": "Monokai.sublime-color-scheme",
  "font_size": 16,
  "font_face": "Menlo",
  "hot_exit": false,
  "remember_open_files": false,
  "update_check": false,
  "ignored_packages": ["Vintage"]
}
JSON
  local lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
  if [[ -x "$lsregister" ]]; then
    "$lsregister" -f "$app" || true
  fi
  # Opening the bundle path does not need a LaunchServices name lookup.
  open "$app" || true
  local appeared=0
  local _
  for _ in $(seq 1 30); do
    if swift /tmp/lux-window-id.swift "Sublime Text" >/dev/null 2>&1; then
      appeared=1
      break
    fi
    sleep 1
  done
  if [[ "$appeared" != 1 ]]; then
    echo "Sublime Text window did not appear" | tee -a "$note"
    return 0
  fi
  "$subl" -n "$fixture" || true
  sleep 2
  if ! swift /tmp/lux-window-id.swift "Sublime Text" >/dev/null 2>&1; then
    echo "Sublime Text window did not appear after opening the fixture" | tee -a "$note"
    return 0
  fi
  local id
  id="$(swift /tmp/lux-window-id.swift "Sublime Text")"
  screencapture -l "$id" -o "$out/sublime-monokai.png" || true
  osascript -e 'tell application "Sublime Text" to quit' >/dev/null 2>&1 || true
  if [[ ! -s "$out/sublime-monokai.png" ]]; then
    echo "Sublime Text window did not appear" | tee -a "$note"
    return 0
  fi
  echo "captured $out/sublime-monokai.png" | tee -a "$note"
  python3 "$root/scripts/monokai_text_pixels.py" \
    "$out/sublime-monokai.png" "#272822" "#F8F8F2" "$out/sublime-monokai-zoom.png" \
    >>"$note" 2>&1 || echo "Sublime pixel check did not pass; the PNG is still the real window" | tee -a "$note"
  if command -v magick >/dev/null 2>&1 && [[ -s "$out/monokai.png" ]]; then
    magick "$out/monokai.png" "$out/sublime-monokai.png" +append "$out/monokai-side-by-side.png" \
      || echo "side-by-side was not written" | tee -a "$note"
    if [[ -s "$out/monokai-side-by-side.png" ]]; then
      echo "captured $out/monokai-side-by-side.png" | tee -a "$note"
    fi
  fi
}

sublime_compare

{
  echo "Lux captures are screencapture of the live window at scale 1, plus an 8x crop."
  echo "VS Code: attempting a real window. A missing PNG means it did not appear."
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
  fi
fi

if [[ -s "$out/vscode-monokai.png" ]]; then
  echo "VS Code window captured" | tee -a "$out/vscode.txt"
else
  echo "VS Code window did not appear. Delivering the Lux scheme captures at 1x with 8x crops, and the Sublime comparison when that window appeared." | tee -a "$out/vscode.txt"
fi

echo "---- schemes ----"
cat "$results"
if [[ "$failed" != 0 ]]; then
  echo "one or more schemes failed the stem check" >&2
  exit 1
fi
