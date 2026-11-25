#!/usr/bin/env bash
# macos-15: a user color scheme with a JSONC comment and a scope list
# loads and paints. The keyword color is not a bundled theme color, so
# the snapshot only passes when that rule was applied.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "theme-jsonc captures run on macOS. This host is $(uname -s)." >&2
  exit 1
fi

out="$root/docs/evidence/theme-jsonc"
rm -rf "$out"
mkdir -p "$out"
report="$out/report.txt"
: > "$report"

fail() {
  echo "FAIL $*" | tee -a "$report" >&2
  if [[ -s "${DRIVE_LOG:-}" ]]; then
    tail -40 "$DRIVE_LOG" >&2 || true
  fi
  exit 1
}

app="$(bash "$root/scripts/stage-lux-app.sh")"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true
mkdir -p "$HOME/.config/lux/themes"
cat > "$HOME/.config/lux/themes/Commented.sublime-color-scheme" << 'EOF'
{
  // Sublime schemes are JSONC and list scopes as an array.
  "name": "Commented",
  "globals": {
    "background": "#112233",
    "foreground": "#EEEEEE"
  },
  "rules": [
    {
      "scope": ["keyword", "storage.type"],
      "foreground": "#FF00AA"
    }
  ]
}
EOF
cat > "$HOME/.config/lux/settings.json" << 'EOF'
{
  "editor.fontSize": 16.0,
  "editor.lineHeight": 1.4,
  "workbench.colorTheme": "Commented"
}
EOF

fixture="$out/sample.rs"
cat > "$fixture" << 'EOF'
fn main() {
    let value: i32 = 1;
}
EOF

drive_init "$app"
trap drive_stop EXIT

dest="$out/commented.png"
drive_launch "$fixture" --snapshot-scale 2 --snapshot-on-usr1 "$dest"

theme=""
tokens=""
for _ in $(seq 1 40); do
  drive_dump
  theme="$(drive_field theme)"
  tokens="$(drive_field syntax_tokens)"
  if [[ "$theme" == "Commented" && -n "$tokens" && "$tokens" != "0" ]]; then
    break
  fi
  sleep 0.25
done
{
  echo "theme=$theme syntax_tokens=$tokens"
} | tee -a "$report"
if [[ "$theme" != "Commented" ]]; then
  fail "user scheme did not load (theme=$theme)"
fi
if [[ -z "$tokens" || "$tokens" == "0" ]]; then
  fail "syntax did not highlight the fixture"
fi

rm -f "$dest"
kill -USR1 "$DRIVE_PID"
ready=0
for _ in $(seq 1 40); do
  if [[ -s "$dest" ]]; then
    ready=1
    break
  fi
  sleep 0.25
done
if [[ "$ready" != 1 ]]; then
  fail "Lux did not write $dest"
fi
python3 "$root/scripts/theme_pixels.py" ink "$dest" "#112233" "#FF00AA" | tee -a "$report"
echo "PASS commented scheme" | tee -a "$report"
