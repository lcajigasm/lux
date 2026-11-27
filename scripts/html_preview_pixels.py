#!/usr/bin/env python3
"""The HTML preview pane is the panel color, with ink, and it changes on edit."""

import struct
import sys
import zlib
from pathlib import Path

PANEL = (0x1E, 0x1F, 0x1C)
EDITOR = (0x27, 0x28, 0x22)


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


def dist(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def pane_left(rows):
    """The preview pane's left edge: the first long run of the panel color
    on a row below the content. The pane's width follows the editor
    layout, so a fixed fraction of the capture misses edits near its left
    edge."""
    row = rows[int(len(rows) * 0.6)]
    run = 0
    for x, pixel in enumerate(row):
        if dist(pixel, PANEL) <= 2:
            run += 1
            if run >= 64:
                return x - run + 1
        else:
            run = 0
    raise SystemExit("no preview pane in the capture")


def pane(rows, x0):
    height = len(rows)
    y0 = 80
    y1 = height - 60
    return [row[x0:] for row in rows[y0:y1]]


def main():
    folder = Path(sys.argv[1])
    rendered = load_png(folder / "rendered.png")
    x0 = pane_left(rendered)
    before = pane(rendered, x0)
    after = pane(load_png(folder / "updated.png"), x0)
    panel = ink = 0
    total = 0
    for row in before:
        for pixel in row:
            total += 1
            if dist(pixel, PANEL) <= 12:
                panel += 1
            elif dist(pixel, EDITOR) > 18:
                ink += 1
    if panel < total * 0.4:
        raise SystemExit(f"preview pane is not the panel color ({panel}/{total})")
    if ink < 30:
        raise SystemExit(f"preview pane has no rendered ink ({ink})")
    changed = 0
    for left, right in zip(before, after):
        for a, b in zip(left, right):
            if a != b:
                changed += 1
    if changed < 8:
        raise SystemExit(f"editing the source left the preview unchanged ({changed})")
    print(f"preview pane x={x0} panel={panel} ink={ink} changed={changed}")


if __name__ == "__main__":
    main()
