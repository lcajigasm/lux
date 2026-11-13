#!/usr/bin/env python3
"""Pixel checks for the sidebar clip captures.

The project header and the status-bar strip inside the sidebar column
are compared across the top, middle, and end of the scroll. They must
match. The rows between them must change, or the tree did not scroll.
"""

import struct
import sys
import zlib
from pathlib import Path

# Capture scale is 2. These are the physical sizes at that scale.
SIDEBAR_W = 220 * 2
TAB_H = 32 * 2
HEADER_H = 28 * 2
STATUS_H = 24 * 2
SIDEBAR = (0x1E, 0x1F, 0x1C)


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
            if chunk[8:13] != b"\x08\x06\x00\x00\x00":
                raise SystemExit(f"{path} is not 8-bit RGBA")
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
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
    return width, height, rows


def band(rows, y0, y1, x0, x1):
    return [rows[y][x0:x1] for y in range(y0, y1)]


def differ(a, b):
    count = 0
    for row_a, row_b in zip(a, b):
        for left, right in zip(row_a, row_b):
            if left != right:
                count += 1
    return count


def ink(block):
    count = 0
    for row in block:
        for pixel in row:
            if max(abs(pixel[0] - SIDEBAR[0]), abs(pixel[1] - SIDEBAR[1]), abs(pixel[2] - SIDEBAR[2])) > 18:
                count += 1
    return count


def main():
    folder = Path(sys.argv[1])
    shots = [load_png(folder / name) for name in ("01-top.png", "02-middle.png", "03-end.png")]
    width, height, _ = shots[0]
    for shot in shots[1:]:
        if shot[0] != width or shot[1] != height:
            raise SystemExit("captures differ in size")
    if width < SIDEBAR_W or height < TAB_H + HEADER_H + STATUS_H + 80:
        raise SystemExit(f"capture {width}x{height} is too small for the sidebar")

    header = (TAB_H, TAB_H + HEADER_H, 0, SIDEBAR_W - 2)
    status = (height - STATUS_H, height, 0, SIDEBAR_W - 2)
    body = (TAB_H + HEADER_H + 2, height - STATUS_H - 2, 8, SIDEBAR_W - 16)

    headers = [band(rows, *header) for _, _, rows in shots]
    statuses = [band(rows, *status) for _, _, rows in shots]
    bodies = [band(rows, *body) for _, _, rows in shots]

    if ink(headers[0]) < 20:
        raise SystemExit("the project header has no title ink")
    header_drift = differ(headers[0], headers[1]) + differ(headers[0], headers[2])
    if header_drift:
        raise SystemExit(f"the header changed while scrolling ({header_drift} pixels)")
    status_drift = differ(statuses[0], statuses[1]) + differ(statuses[0], statuses[2])
    if status_drift:
        raise SystemExit(f"the status bar changed under the sidebar ({status_drift} pixels)")
    moved = differ(bodies[0], bodies[1])
    moved_end = differ(bodies[1], bodies[2])
    if moved < 200 or moved_end < 200:
        raise SystemExit(f"the tree rows did not move (middle {moved}, end {moved_end})")
    print(f"header stable, status stable, rows moved {moved} then {moved_end}")


if __name__ == "__main__":
    main()
