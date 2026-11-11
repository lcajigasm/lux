#!/usr/bin/env python3
"""Pixel check for grayscale text on a real window capture.

A scale-1 LCD mask paints a colored fringe beside a neutral stem. On
docs/compare/monokai-side-by-side.png (macos-15 run 41348580486) the
pixel to the right of a #F8F8F2 stem is (133, 89, 38). A grayscale ramp
between two low-chroma colors stays low-chroma. This fails the image
when that fringe is present, and writes an 8× nearest-neighbor crop.
"""

import sys
import zlib
import struct
from pathlib import Path


def parse_hex(text):
    raw = text.strip().lstrip("#")
    if len(raw) != 6:
        raise SystemExit(f"bad color {text}")
    return tuple(int(raw[i : i + 2], 16) for i in (0, 2, 4))


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def load_png(path):
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = None
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
    i = 0
    for _ in range(height):
        filt = raw[i]
        i += 1
        row = bytearray(raw[i : i + stride])
        i += stride
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
            raise SystemExit(f"unsupported PNG filter {filt}")
        prev = row
        if channels == 4:
            rows.append(bytes(row[i] for i in range(len(row)) if i % 4 != 3))
        else:
            rows.append(bytes(row))
    return width, height, rows


def pixel(rows, x, y):
    i = x * 3
    row = rows[y]
    return (row[i], row[i + 1], row[i + 2])


def spread(color):
    return max(color) - min(color)


def near(color, target, tol):
    return all(abs(color[i] - target[i]) <= tol for i in range(3))


def check(path, background, foreground, zoom_path):
    width, height, rows = load_png(path)
    stems = []
    for y in range(height):
        for x in range(width):
            color = pixel(rows, x, y)
            # Grayscale antialiasing pulls a light-theme stem off the
            # exact foreground. On macos-15 run 42986567243 Solarized
            # Light's #657b83 had 18 pixels inside 8 and 40 inside 12,
            # and those 18 had no colored fringe. Dark themes stay in
            # the hundreds at either tolerance. 16 matches tens of
            # thousands of near-white chrome pixels, so 12 is the
            # widest tolerance that still names a stem.
            if near(color, foreground, 12) and spread(color) <= 36:
                stems.append((x, y))
    if len(stems) < 20:
        raise SystemExit(f"{path}: found {len(stems)} stem pixels near {foreground}, want >= 20")
    fringes = []
    for x, y in stems:
        for dx in (-3, -2, -1, 1, 2, 3):
            nx = x + dx
            if nx < 0 or nx >= width:
                continue
            color = pixel(rows, nx, y)
            if near(color, background, 8) or near(color, foreground, 12):
                continue
            if spread(color) <= 45:
                continue
            # The fringe sits on the editor well. A traffic-light or
            # title-bar pixel can be chromatic beside white chrome, and
            # that white is not the editor background.
            if not any(
                0 <= nx + ox < width
                and 0 <= y + oy < height
                and near(pixel(rows, nx + ox, y + oy), background, 10)
                for ox in (-2, -1, 0, 1, 2)
                for oy in (-2, -1, 0, 1, 2)
            ):
                continue
            fringes.append((nx, y, color, (x, y)))
            if len(fringes) >= 8:
                break
        if len(fringes) >= 8:
            break
    if fringes:
        shown = ", ".join(f"{x},{y}={color}" for x, y, color, _ in fringes[:6])
        raise SystemExit(f"{path}: colored fringe beside a neutral stem: {shown}")
    # Background must actually be on the frame. Sample the modal-ish
    # corners of the editor by counting exact background hits.
    hits = 0
    for y in range(0, height, 3):
        for x in range(0, width, 3):
            if near(pixel(rows, x, y), background, 2):
                hits += 1
    if hits < 50:
        raise SystemExit(f"{path}: background {background} appears {hits} times, want >= 50")
    write_zoom(rows, width, height, stems, zoom_path)
    print(f"PASS {path} stems={len(stems)} background_hits={hits} zoom={zoom_path}")


def write_zoom(rows, width, height, stems, dest):
    sx, sy = stems[len(stems) // 2]
    x0, y0 = max(0, sx - 18), max(0, sy - 10)
    x1, y1 = min(width, sx + 22), min(height, sy + 12)
    scale = 8
    out_w, out_h = (x1 - x0) * scale, (y1 - y0) * scale
    raw = bytearray()
    for y in range(out_h):
        raw.append(0)
        src_y = y0 + y // scale
        row = rows[src_y]
        for x in range(out_w):
            src_x = x0 + x // scale
            i = src_x * 3
            raw.extend(row[i : i + 3])
    def chunk(kind, payload):
        crc = zlib.crc32(kind + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", crc)
    ihdr = struct.pack(">IIBBBBB", out_w, out_h, 8, 2, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b"")
    Path(dest).write_bytes(png)


def main():
    if len(sys.argv) != 5:
        raise SystemExit("usage: monokai_text_pixels.py IMAGE #RRGGBB #RRGGBB ZOOM.png")
    _, image, background, foreground, zoom = sys.argv
    check(image, parse_hex(background), parse_hex(foreground), zoom)


if __name__ == "__main__":
    main()
