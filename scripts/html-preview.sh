#!/usr/bin/env bash
# Real Lux.app on macOS. HTML preview sits beside the source and updates
# when the source changes.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/html-preview"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/html-preview.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"

page="$HOME/page.html"
cat > "$page" << 'EOF'
<h1>Hello</h1>
<p>A paragraph with a <a href="https://example.com">link</a>.</p>
<pre>code</pre>
EOF

drive_init "$root/target/Lux.app"
drive_launch "$page" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/rendered.png" \
  --snapshot-on-usr1 "$out/updated.png"

opened=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field language)" == "HTML" ]]; then
    opened=1
    break
  fi
  sleep 0.3
done
if [[ "$opened" != 1 ]]; then
  echo "page.html did not open as HTML" >&2
  cat "$DRIVE_PROBE" >&2 || true
  exit 1
fi

# Shift+Command+V. V is virtual key 9.
drive_key tap 9 "cmd,shift"
sleep 0.4
drive_dump
blocks="$(drive_field preview_blocks)"
if [[ "$(drive_field preview)" != "1" ]]; then
  echo "preview did not open" >&2
  exit 1
fi
for kind in "heading:Hello" "link" "code:code"; do
  if [[ "$blocks" != *"$kind"* ]]; then
    echo "preview blocks missing $kind: $blocks" >&2
    exit 1
  fi
done

capture() {
  local dest="$1"
  kill -USR1 "$DRIVE_PID"
  local attempt
  for attempt in $(seq 1 40); do
    if [[ -s "$dest" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "snapshot missing: $dest" >&2
  exit 1
}
capture "$out/rendered.png"

# Four rights land after `<h1>`, still on the heading text.
for _ in 1 2 3 4; do
  drive_key tap 124 ""
  sleep 0.05
done
drive_key type " LIVE"
sleep 0.4
drive_dump
blocks="$(drive_field preview_blocks)"
text="$(drive_field text)"
if [[ "$text" != *"LIVE"* ]]; then
  echo "the edit did not land in the source: $text" >&2
  exit 1
fi
if [[ "$blocks" != *"heading:LIVEHello"* ]]; then
  echo "the preview did not follow the edit: $blocks" >&2
  exit 1
fi
if [[ "$(drive_field preview)" != "1" ]]; then
  echo "preview closed during the edit" >&2
  exit 1
fi
capture "$out/updated.png"

python3 "$root/scripts/html_preview_pixels.py" "$out"

{
  echo "html preview"
  echo "blocks after edit: $blocks"
  ls -1 "$out"/*.png
} | tee "$out/html-preview.txt"

drive_stop
