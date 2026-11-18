#!/usr/bin/env bash
# Rasterize design/icons/*.svg with librsvg, then place each icon on a
# light and a dark canvas. sips makes the 128 and 32 sizes from that PNG.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
src="$root/design/icons"
out="$root/docs/evidence/icon-proposals"
mkdir -p "$out"

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert is not installed" >&2
  exit 1
fi

for name in slot meter corner; do
  raw="$out/$name-raw.png"
  rsvg-convert -w 1024 -h 1024 "$src/lux-$name.svg" -o "$raw"
  for bg in light dark; do
    swift "$src/compose.swift" "$raw" "$bg" "$out/$name-$bg-1024.png"
    sips -z 128 128 "$out/$name-$bg-1024.png" --out "$out/$name-$bg-128.png" >/dev/null
    sips -z 32 32 "$out/$name-$bg-1024.png" --out "$out/$name-$bg-32.png" >/dev/null
  done
  rm -f "$raw"
done

swift "$src/contact.swift" "$out/contact.png" \
  "$out/slot-light-1024.png" "$out/slot-dark-1024.png" \
  "$out/meter-light-1024.png" "$out/meter-dark-1024.png" \
  "$out/corner-light-1024.png" "$out/corner-dark-1024.png" \
  "$out/slot-light-128.png" "$out/slot-dark-128.png" \
  "$out/meter-light-128.png" "$out/meter-dark-128.png" \
  "$out/corner-light-128.png" "$out/corner-dark-128.png" \
  "$out/slot-light-32.png" "$out/slot-dark-32.png" \
  "$out/meter-light-32.png" "$out/meter-dark-32.png" \
  "$out/corner-light-32.png" "$out/corner-dark-32.png"

find "$out" -type f -name '*.png' -print | sort
