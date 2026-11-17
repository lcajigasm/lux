#!/usr/bin/env python3
"""Fail when a fence preview paints indent and word spaces on top of each other.

A highlighted fence used to lay each color run out on its own. A run that
is only spaces is trailing whitespace, and its width is 0, so `let` sat
where `fn` ended. This checks the captured window: some lines start a
step to the right, and some lines have a gap the size of a space.
"""

import struct
import sys
import zlib
from pathlib import Path


def load_png(path):
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = color = None
    idat = b""
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, bit, color, *_ = struct.unpack(">IIBBBBB", chunk)
            if bit != 8 or color not in (2, 6):
                raise SystemExit(f"{path} is not 8-bit RGB or RGBA")
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
    raw = zlib.decompress(idat)
    channels = 4 if color == 6 else 3
    stride = width * channels
    rows = []
    prev = bytearray(stride)
    index = 0
    for _ in range(height):
        filt = raw[index]
        index += 1
        row = bytearray(raw[index : index + stride])
        index += stride
        if filt == 1:
            for x in range(stride):
                left = row[x - channels] if x >= channels else 0
                row[x] = (row[x] + left) & 255
        elif filt == 2:
            for x in range(stride):
                row[x] = (row[x] + prev[x]) & 255
        elif filt == 3:
            for x in range(stride):
                left = row[x - channels] if x >= channels else 0
                row[x] = (row[x] + ((left + prev[x]) // 2)) & 255
        elif filt == 4:
            for x in range(stride):
                left = row[x - channels] if x >= channels else 0
                up_left = prev[x - channels] if x >= channels else 0
                row[x] = (row[x] + paeth(left, prev[x], up_left)) & 255
        elif filt != 0:
            raise SystemExit(f"{path} uses PNG filter {filt}")
        prev = row
        rows.append(bytes(row))
    return width, height, channels, rows


def paeth(left, up, up_left):
    estimate = left + up - up_left
    distances = (
        abs(estimate - left),
        abs(estimate - up),
        abs(estimate - up_left),
    )
    return (left, up, up_left)[distances.index(min(distances))]


def pixel(rows, channels, x, y):
    offset = x * channels
    return tuple(rows[y][offset : offset + 3])


def chebyshev(left, right):
    return max(abs(left[i] - right[i]) for i in range(3))


def measure(path):
    width, height, channels, rows = load_png(path)
    counts = {}
    for y in range(int(height * 0.20), int(height * 0.85)):
        for x in range(int(width * 0.15), int(width * 0.85)):
            color = pixel(rows, channels, x, y)
            counts[color] = counts.get(color, 0) + 1
    background = max(counts, key=counts.get)
    y0 = int(height * 0.12)
    y1 = int(height * 0.90)
    ink_rows = []
    for y in range(y0, y1):
        xs = [
            x
            for x in range(width)
            if chebyshev(pixel(rows, channels, x, y), background) >= 40
        ]
        if len(xs) >= 8:
            ink_rows.append((y, xs))
    lines = []
    current = []
    last_y = None
    for y, xs in ink_rows:
        if last_y is not None and y > last_y + 2 and current:
            lines.append(current)
            current = []
        current.append(xs)
        last_y = y
    if current:
        lines.append(current)
    measured = []
    for line in lines:
        xs = [x for row in line for x in row]
        if len(xs) < 20:
            continue
        left = min(xs)
        present = set(xs)
        gaps = []
        run = False
        gap = 0
        for x in range(left, max(xs) + 1):
            if x in present:
                if gap >= 3 and run:
                    gaps.append(gap)
                gap = 0
                run = True
            elif run:
                gap += 1
        measured.append((left, gaps))
    return background, measured


def check(path):
    _background, lines = measure(path)
    if len(lines) < 6:
        raise SystemExit(f"{path} has {len(lines)} text lines, want at least 6")
    lefts = [left for left, _gaps in lines]
    base = min(lefts)
    indented = [left for left in lefts if left >= base + 8]
    gaps = [gap for _left, row_gaps in lines for gap in row_gaps if gap >= 3]
    if len(indented) < 3:
        raise SystemExit(
            f"{path} indent collapsed: {len(indented)} lines start 8px right of {base}"
        )
    if len(gaps) < 3:
        raise SystemExit(f"{path} word spaces collapsed: {len(gaps)} gaps")
    print(
        f"indent lines {len(indented)} base {base} "
        f"step {min(indented) - base} gaps {len(gaps)}"
    )


def write_png(path, width, height, rgb_rows):
    def chunk(kind, payload):
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    raw = b"".join(b"\x00" + row for row in rgb_rows)
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )
    Path(path).write_bytes(png)


def block(row, x0, x1, color):
    for x in range(x0, x1):
        row[x * 3 : x * 3 + 3] = bytes(color)


def synthetic(indented):
    width, height = 240, 160
    background = (0x27, 0x28, 0x22)
    ink = (0xF8, 0xF8, 0xF2)
    rows = []
    for y in range(height):
        row = bytearray(background * width)
        # Six body lines, past the top chrome band the checker skips.
        rows.append(row)
    for slot, y in enumerate((40, 52, 64, 76, 88, 100)):
        origin = 30
        if indented and slot % 2 == 1:
            origin = 30 + 16
        for dy in range(8):
            block(rows[y + dy], origin, origin + 14, ink)
            block(rows[y + dy], origin + 22, origin + 50, ink)
    return width, height, rows


def self_test():
    path = Path(__file__).with_name("_fence_indent_selftest.png")
    width, height, rows = synthetic(True)
    write_png(path, width, height, [bytes(row) for row in rows])
    try:
        check(path)
    finally:
        path.unlink(missing_ok=True)
    width, height, rows = synthetic(False)
    write_png(path, width, height, [bytes(row) for row in rows])
    try:
        try:
            check(path)
        except SystemExit as stopped:
            if "indent collapsed" not in str(stopped):
                raise
            print("self-test collapsed indent rejected")
            return
        raise SystemExit("self-test accepted a preview with no indent")
    finally:
        path.unlink(missing_ok=True)


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        self_test()
        return
    if len(sys.argv) != 2:
        raise SystemExit("usage: fence_indent.py SNAPSHOT.png")
    check(sys.argv[1])


if __name__ == "__main__":
    main()
