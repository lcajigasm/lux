#!/usr/bin/env python3
"""Pixel checks for the gutter clip captures.

The margin strip may contain only the gutter background and line-number
ink. After a horizontal scroll that strip is unchanged. After a vertical
scroll the numbers and the text move together (within one pixel). A
full-height stroke in the text area is a loose guide.
"""

import struct
import sys
import zlib
from pathlib import Path

BG = (0x27, 0x28, 0x22)
INK = 16


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
        rows.append(
            [tuple(row[x * 4 : x * 4 + 3]) for x in range(width)]
        )
        src += stride
    return width, height, rows


def dist(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def ink(pixel):
    return dist(pixel, BG) > INK


def editor_band(rows):
    height = len(rows)
    width = len(rows[0])
    top = None
    for y in range(height):
        near = sum(1 for x in range(0, width, 4) if dist(rows[y][x], BG) <= 8)
        if near > (width // 4) * 0.5:
            top = y
            break
    if top is None:
        raise SystemExit("no editor background")
    bottom = height
    for y in range(height - 1, top, -1):
        near = sum(1 for x in range(0, width, 4) if dist(rows[y][x], BG) <= 8)
        if near > (width // 4) * 0.5:
            bottom = y + 1
            break
    return top, bottom


def text_origin(rows, top, bottom):
    """Left edge of the buffer text, not the minimap.

    A full-width line is one solid bar in the minimap, and the longest ink
    run in the text is a single "M". Group the spans and take the widest
    group. The minimap sits in the right 220px and is left out.
    """
    width = len(rows[0])
    counts = [0] * width
    for y in range(top, bottom):
        for x in range(width):
            if ink(rows[y][x]):
                counts[x] += 1
    cutoff = max(0, width - 220)
    spans = []
    start = None
    gap = 0
    end = 0
    for x in range(cutoff):
        if counts[x] >= 2:
            if start is None:
                start = x
            gap = 0
            end = x + 1
        elif start is not None:
            gap += 1
            if gap > 2:
                spans.append((start, end))
                start = None
    if start is not None:
        spans.append((start, end))
    if not spans:
        raise SystemExit("no text run found")
    groups = [[spans[0]]]
    for span in spans[1:]:
        if span[0] - groups[-1][-1][1] > 16:
            groups.append([span])
        else:
            groups[-1].append(span)
    text = max(groups, key=lambda group: group[-1][1] - group[0][0])
    return text[0][0]


def digit_right(rows, top, bottom, text_left):
    right = 0
    found = False
    for y in range(top, bottom):
        for x in range(text_left):
            if ink(rows[y][x]):
                found = True
                right = max(right, x)
    if not found:
        raise SystemExit("line numbers are not visible")
    return right


def first_ink_row(rows, x0, x1, y0, y1):
    for y in range(y0, y1):
        for x in range(x0, x1):
            if ink(rows[y][x]):
                return y
    return None


def margin_equal(a, b, top, bottom, text_left):
    for y in range(top, bottom):
        for x in range(text_left):
            if a[y][x] != b[y][x]:
                return False
    return True


def stray_ink(rows, top, bottom, left, right):
    # The caret is a few pixels wide and blinks in the padding. Buffer text
    # that crosses into the margin is a much longer run.
    if left >= right:
        return False
    for y in range(top, bottom):
        run = 0
        for x in range(left, right):
            if ink(rows[y][x]):
                run += 1
                if run >= 12:
                    return True
            else:
                run = 0
    return False


def ink_clusters(rows, top, bottom, x0, x1):
    groups = []
    start = None
    for y in range(top, bottom):
        on = any(ink(rows[y][x]) for x in range(x0, x1))
        if on and start is None:
            start = y
        if not on and start is not None:
            groups.append((start, y))
            start = None
    if start is not None:
        groups.append((start, bottom))
    return groups


def check(name, rows, top, bottom, origin, numbers):
    if stray_ink(rows, top, bottom, numbers + 2, origin):
        raise SystemExit(f"{name}: text ink in the margin")
    if first_ink_row(rows, 0, numbers + 1, top, bottom) is None:
        raise SystemExit(f"{name}: line numbers are not visible")


def main():
    folder = Path(sys.argv[1])
    before = load_png(folder / "01-before.png")[2]
    shifted = load_png(folder / "02-after-shift-wheel.png")[2]
    delta_x = load_png(folder / "03-after-delta-x.png")[2]
    vertical = load_png(folder / "04-after-vertical.png")[2]
    wrapped = load_png(folder / "05-wrap-on.png")[2]
    top, bottom = editor_band(before)
    origin = text_origin(before, top, bottom)
    numbers = digit_right(before, top, bottom, origin)
    print(f"editor {top}:{bottom} text_left={origin} digit_right={numbers}")
    for name, rows in (
        ("01-before.png", before),
        ("02-after-shift-wheel.png", shifted),
        ("03-after-delta-x.png", delta_x),
        ("04-after-vertical.png", vertical),
        ("05-wrap-on.png", wrapped),
    ):
        check(name, rows, top, bottom, origin, numbers)
    # Compare the line-number strip, not the padding where the caret blinks.
    gutter_right = numbers + 1
    if not margin_equal(before, shifted, top, bottom, gutter_right):
        raise SystemExit("shift+wheel changed the margin strip")
    if not margin_equal(before, delta_x, top, bottom, gutter_right):
        raise SystemExit("delta-x wheel changed the margin strip")
    before_delta = first_ink_row(before, origin, origin + 30, top, bottom) - first_ink_row(
        before, 0, numbers + 1, top, bottom
    )
    after_delta = first_ink_row(vertical, origin, origin + 30, top, bottom) - first_ink_row(
        vertical, 0, numbers + 1, top, bottom
    )
    if abs(before_delta - after_delta) > 1:
        raise SystemExit(
            f"vertical sync {before_delta}px then {after_delta}px"
        )
    plain = ink_clusters(before, top, bottom, 0, numbers + 1)
    wrapped_numbers = ink_clusters(wrapped, top, bottom, 0, numbers + 1)
    if len(plain) < 2 or len(wrapped_numbers) < 2:
        raise SystemExit("need two numbered lines to judge wrap")
    plain_gap = plain[1][0] - plain[0][0]
    wrap_gap = wrapped_numbers[1][0] - wrapped_numbers[0][0]
    if wrap_gap < plain_gap * 2:
        raise SystemExit(f"wrap did not stack the long line ({plain_gap} -> {wrap_gap})")
    print("gutter pixels ok")


if __name__ == "__main__":
    main()
