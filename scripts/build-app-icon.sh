#!/usr/bin/env bash
# Rasterize packaging/macos/lux-corner.svg into the macOS iconset and
# lux.icns. On macOS, iconutil packs the icon. Elsewhere, the same PNGs
# are packed into an icns container so the file exists in the tree.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
svg="$root/packaging/macos/lux-corner.svg"
iconset="$root/packaging/macos/Lux.iconset"
icns="$root/packaging/macos/lux.icns"

if [[ ! -f "$svg" ]]; then
  echo "missing $svg" >&2
  exit 1
fi
if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert is not installed" >&2
  exit 1
fi

mkdir -p "$iconset"
render() {
  local px="$1"
  local dest="$2"
  rsvg-convert -w "$px" -h "$px" "$svg" -o "$dest"
}

render 16 "$iconset/icon_16x16.png"
render 32 "$iconset/icon_16x16@2x.png"
render 32 "$iconset/icon_32x32.png"
render 64 "$iconset/icon_32x32@2x.png"
render 128 "$iconset/icon_128x128.png"
render 256 "$iconset/icon_128x128@2x.png"
render 256 "$iconset/icon_256x256.png"
render 512 "$iconset/icon_256x256@2x.png"
render 512 "$iconset/icon_512x512.png"
render 1024 "$iconset/icon_512x512@2x.png"

if command -v iconutil >/dev/null 2>&1; then
  iconutil -c icns "$iconset" -o "$icns"
else
  python3 - "$iconset" "$icns" << 'PY'
import struct
import sys
from pathlib import Path

iconset = Path(sys.argv[1])
out = Path(sys.argv[2])
order = [
    ("icp4", "icon_16x16.png", 16),
    ("ic11", "icon_16x16@2x.png", 32),
    ("icp5", "icon_32x32.png", 32),
    ("icp6", "icon_32x32@2x.png", 64),
    ("ic12", "icon_32x32@2x.png", 64),
    ("ic07", "icon_128x128.png", 128),
    ("ic13", "icon_128x128@2x.png", 256),
    ("ic08", "icon_256x256.png", 256),
    ("ic14", "icon_256x256@2x.png", 512),
    ("ic09", "icon_512x512.png", 512),
    ("ic10", "icon_512x512@2x.png", 1024),
]
chunks = []
for kind, name, side in order:
    data = (iconset / name).read_bytes()
    if data[16:24] != struct.pack(">II", side, side):
        raise SystemExit(f"{name} is not {side}x{side}")
    chunks.append(kind.encode("ascii") + struct.pack(">I", 8 + len(data)) + data)
body = b"".join(chunks)
blob = b"icns" + struct.pack(">I", 8 + len(body)) + body
out.write_bytes(blob)
print(f"packed {out} ({len(blob)} bytes)")
PY
fi

test -s "$icns"
echo "wrote $icns"
