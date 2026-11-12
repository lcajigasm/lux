#!/usr/bin/env python3
"""Measure selection contrast on a real Lux snapshot.

The floor matches lux_ui::SELECTION_CONTRAST_MIN (1.30). Sublime Monokai
#49483E on #272822 is 1.61. The old #49483E88 paint was 1.28. The script
also writes the measured hex and a side-by-side crop of the two captures.
"""

import struct
import sys
import zlib
from pathlib import Path

FLOOR = 1.30
MONOKAI_SELECTION = (0x49, 0x48, 0x3E)
MONOKAI_BACKGROUND = (0x27, 0x28, 0x22)


def lin(channel):
    value = channel / 255.0
    if value <= 0.03928:
        return value / 12.92
    return ((value + 0.055) / 1.055) ** 2.4


def contrast(left, right):
    def luminance(rgb):
        return 0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])

    hi = luminance(left)
    lo = luminance(right)
    if hi < lo:
        hi, lo = lo, hi
    return (hi + 0.05) / (lo + 0.05)


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
    raw = rows[y][offset : offset + 3]
    return (raw[0], raw[1], raw[2])


def chebyshev(left, right):
    return max(abs(left[i] - right[i]) for i in range(3))


def mode(colors):
    counts = {}
    for color in colors:
        counts[color] = counts.get(color, 0) + 1
    return max(counts, key=counts.get), counts[max(counts, key=counts.get)]


def measure(path):
    width, height, channels, rows = load_png(path)
    background_samples = []
    for y in range(int(height * 0.40), int(height * 0.80)):
        for x in range(int(width * 0.30), int(width * 0.80)):
            background_samples.append(pixel(rows, channels, x, y))
    background, _ = mode(background_samples)
    # Skip the tab strip and the status bar. The status bar (#414339) is
    # close to the selection and would otherwise win the row search.
    y0 = int(height * 0.04)
    y1 = int(height * 0.72)
    band_score = [0] * height
    for y in range(y0, y1):
        score = 0
        for x in range(width):
            distance = chebyshev(pixel(rows, channels, x, y), background)
            if 12 <= distance <= 90:
                score += 1
        band_score[y] = score
    best_y = 0
    best = -1
    window = 12
    for y in range(0, max(1, height - window)):
        total = sum(band_score[y : y + window])
        if total > best:
            best = total
            best_y = y
    chosen = []
    for y in range(best_y, min(height, best_y + window)):
        for x in range(width):
            color = pixel(rows, channels, x, y)
            distance = chebyshev(color, background)
            if 12 <= distance <= 90:
                chosen.append(color)
    if len(chosen) < 40:
        raise SystemExit(f"{path} has no selection band ({len(chosen)} pixels)")
    selection, count = mode(chosen)
    ratio = contrast(selection, background)
    return {
        "path": path,
        "width": width,
        "height": height,
        "rows": rows,
        "channels": channels,
        "background": background,
        "selection": selection,
        "count": count,
        "ratio": ratio,
        "band_y": best_y,
    }


def hex_of(rgb):
    return "#" + "".join(f"{channel:02X}" for channel in rgb)


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
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    Path(path).write_bytes(png)


def crop_rows(measured, rows_high):
    channels = measured["channels"]
    width = measured["width"]
    height = min(rows_high, measured["height"])
    out = []
    for y in range(height):
        row = bytearray()
        source = measured["rows"][y]
        for x in range(width):
            offset = x * channels
            row.extend(source[offset : offset + 3])
        out.append(bytes(row))
    return width, out


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: selection_contrast.py LINE.png WORD.png OUT_DIR")
    line_path, word_path, out_dir = sys.argv[1:]
    out = Path(out_dir)
    line = measure(line_path)
    word = measure(word_path)
    lines = []
    for label, sample in (("line", line), ("word", word)):
        if sample["ratio"] < FLOOR:
            raise SystemExit(
                f"{label} selection {hex_of(sample['selection'])} on "
                f"{hex_of(sample['background'])} is {sample['ratio']:.3f}:1, below {FLOOR}"
            )
        if chebyshev(sample["selection"], MONOKAI_SELECTION) > 4:
            raise SystemExit(
                f"{label} selection {hex_of(sample['selection'])} is not Monokai #49483E"
            )
        if chebyshev(sample["background"], MONOKAI_BACKGROUND) > 4:
            raise SystemExit(
                f"{label} background {hex_of(sample['background'])} is not Monokai #272822"
            )
        lines.append(
            f"{label} selection {hex_of(sample['selection'])} "
            f"background {hex_of(sample['background'])} "
            f"contrast {sample['ratio']:.3f} pixels {sample['count']}"
        )
    lines.append(f"floor {FLOOR:.2f}")
    lines.append("sublime #49483E on #272822 is 1.61")
    lines.append("vscode #878b9180 paints about 2.13")
    text = "\n".join(lines) + "\n"
    (out / "colors.txt").write_text(text)
    print(text, end="")
    crop_h = 280
    left_w, left_rows = crop_rows(line, crop_h)
    right_w, right_rows = crop_rows(word, crop_h)
    height = min(len(left_rows), len(right_rows))
    gap = b"\x00" * (8 * 3)
    stitched = []
    for y in range(height):
        stitched.append(left_rows[y][: left_w * 3] + gap + right_rows[y][: right_w * 3])
    write_png(out / "selection-side-by-side.png", left_w + 8 + right_w, height, stitched)


if __name__ == "__main__":
    main()
