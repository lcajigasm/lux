#!/usr/bin/env bash
# E1: a scale-2 capture of the real Lux window. The offscreen renderer
# snapshot is not this measurement. If the runner display stays 1×, the
# result is solo-Mac and the job still exits 0.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
note="$out/e1-hidpi.txt"
: > "$note"
log() { echo "$*" | tee -a "$note"; }

log "--> display before any change"
system_profiler SPDisplaysDataType >> "$note" 2>&1 || true

export HOMEBREW_NO_AUTO_UPDATE=1
export NONINTERACTIVE=1
if ! brew install displayplacer >> "$note" 2>&1; then
  log "RESULT solo-mac"
  log "displayplacer did not install. The runner framebuffer was not switched to HiDPI."
  exit 0
fi

placer="$(command -v displayplacer || true)"
log "--> displayplacer list"
if ! "$placer" list >> "$note" 2>&1; then
  log "RESULT solo-mac"
  log "displayplacer list failed. No virtual HiDPI mode was applied."
  exit 0
fi

# A scaled mode looks like: id:<uuid> res:1440x900 hz:60 color_depth:8 scaling:on
mode="$(awk '
  /scaling:on/ && /res:/ {
    print
    exit
  }
' "$note" || true)"
if [[ -z "$mode" ]]; then
  log "RESULT solo-mac"
  log "displayplacer listed no mode with scaling:on. The runner cannot show a 2× framebuffer, so a screencapture of the window cannot be scale 2."
  exit 0
fi

log "--> trying mode: $mode"
if ! "$placer" "$mode" >> "$note" 2>&1; then
  log "RESULT solo-mac"
  log "displayplacer refused that mode. E1 stays solo-Mac."
  exit 0
fi

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT
file="$DRIVE_WORK/hidpi.rs"
printf 'fn main() {}\n' > "$file"
drive_launch "$file"
sleep 0.5
shot="$out/e1-window.png"
# screencapture of the whole display; the window pixels are the real framebuffer.
screencapture -x "$shot"
width="$(sips -g pixelWidth "$shot" | awk '/pixelWidth/ {print $2}')"
log "--> capture width $width"
if [[ -n "$width" && "$width" -ge 2000 ]]; then
  log "RESULT covered"
  log "The display capture is ${width}px wide after the scaled mode."
  exit 0
fi
log "RESULT solo-mac"
log "After displayplacer the capture is ${width:-0}px wide, which is not a 2× window. E1 stays solo-Mac."
exit 0
