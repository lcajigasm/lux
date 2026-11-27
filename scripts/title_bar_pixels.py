#!/usr/bin/env python3
"""Pixel checks on a screencapture of the Lux window (unified title bar).

usage: title_bar_pixels.py PNG WINDOW_WIDTH_PT

The traffic lights sit at the left of the 32 pt tab row. Each of the three
14-16 pt buttons must hold pixels clearly apart from the tab-bar color,
which is sampled from empty tab-row space at the right end of the row, and
must be centered on the row: its vertical center at 16 pt from the window
top, within 1.5 pt. AppKit's own placement for a 28 pt title bar puts the
center near 14 pt, so a button AppKit moved back fails.
"""

import struct
import sys
import zlib
from pathlib import Path


def load(path: Path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos, idat = 8, b""
    width = height = channels = None
    while pos + 8 <= len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, color = struct.unpack(">IIBB", chunk[:10])
            if depth != 8 or color not in (2, 6) or chunk[12] != 0:
                raise SystemExit(f"{path}: unsupported PNG (depth {depth}, color {color})")
            channels = 4 if color == 6 else 3
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
    raw = zlib.decompress(idat)
    stride = width * channels
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(height):
        kind = raw[i]
        line = bytearray(raw[i + 1 : i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - channels] if x >= channels else 0
            b = prev[x]
            c = prev[x - channels] if x >= channels else 0
            if kind == 1:
                line[x] = (line[x] + a) & 0xFF
            elif kind == 2:
                line[x] = (line[x] + b) & 0xFF
            elif kind == 3:
                line[x] = (line[x] + (a + b) // 2) & 0xFF
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[x] = (line[x] + pred) & 0xFF
        rows.append(bytes(line))
        prev = line
    return width, height, channels, rows


ROW_PT = 32.0
CENTER_TOLERANCE_PT = 1.5
# Strongest-difference floor for "visible", and the floor for a pixel to
# count as part of a button when finding its vertical extent.
VISIBLE = 90
PART_OF_BUTTON = 60


def main() -> None:
    path, window_pt = Path(sys.argv[1]), float(sys.argv[2])
    width, _height, channels, rows = load(path)
    scale = width / window_pt

    def px_at(x: int, y: int):
        return rows[y][x * channels : x * channels + 3]

    bar = px_at(int((window_pt - 20) * scale), int(ROW_PT / 2 * scale))
    print(f"tab bar color {bar.hex()} at scale {scale:g}")
    # Rows 2..30 pt: clear of the window's top-edge highlight and of the
    # tab row's bottom separator, wide enough for either placement.
    y_lo, y_hi = int(2 * scale), int((ROW_PT - 2) * scale)
    for name, left in (("close", 7), ("minimize", 30), ("zoom", 53)):
        best, top, bottom = 0, None, None
        for y in range(y_lo, y_hi):
            for x in range(int(left * scale), int((left + 18) * scale)):
                p = px_at(x, y)
                diff = sum(abs(p[i] - bar[i]) for i in range(3))
                best = max(best, diff)
                if diff >= PART_OF_BUTTON:
                    top = y if top is None else top
                    bottom = y
        print(f"{name}: strongest difference from the tab bar {best}")
        if best < VISIBLE or top is None:
            raise SystemExit(f"{path}: the {name} button does not stand out from the tab bar")
        center = (top + bottom + 1) / 2 / scale
        print(f"{name}: vertical center {center:.2f} pt (rows {top}..{bottom} px)")
        if abs(center - ROW_PT / 2) > CENTER_TOLERANCE_PT:
            raise SystemExit(
                f"{path}: the {name} button is centered at {center:.2f} pt, "
                f"want {ROW_PT / 2:g} +/- {CENTER_TOLERANCE_PT:g} pt"
            )


if __name__ == "__main__":
    main()
