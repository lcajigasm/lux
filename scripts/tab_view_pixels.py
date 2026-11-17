#!/usr/bin/env python3
"""Fail when a Plain Text capture still shows diagnostic red."""

import struct
import sys
import zlib
from pathlib import Path


def load_png(path: Path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = None
    idat = b""
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height = struct.unpack(">II", chunk[:8])
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
    if width is None or height is None:
        raise SystemExit(f"{path} has no image header")
    raw = zlib.decompress(idat)
    stride = width * 4
    rows = []
    src = 0
    for _ in range(height):
        if raw[src] != 0:
            raise SystemExit(f"{path} uses a PNG filter")
        src += 1
        row = raw[src : src + stride]
        rows.append([tuple(row[x * 4 : x * 4 + 3]) for x in range(width)])
        src += stride
    return rows


def main():
    path = Path(sys.argv[1])
    rows = load_png(path)
    height = len(rows)
    y0 = int(height * 0.08)
    y1 = int(height * 0.94)
    red = 0
    for row in rows[y0:y1]:
        for red_ch, green, blue in row:
            if red_ch > 200 and green < 80 and blue < 80:
                red += 1
    print(f"red_pixels={red} file={path.name}")
    if red >= 8:
        raise SystemExit(
            f"{path.name} still has diagnostic red ({red} pixels). "
            "The previous buffer's marks are painted on this one."
        )


if __name__ == "__main__":
    main()
