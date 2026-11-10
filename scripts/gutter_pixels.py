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


def text_body_x(rows, origin, top, bottom, numbers):
    """First text column that is not the caret.

    The caret is a few pixels wide and starts above the em box. A vertical
    scroll moves that line away, so a sample that includes it reports the
    text and the line numbers out of sync. A raised region wider than the
    caret is the text itself and stays in the sample.
    """
    number_y = first_ink_row(rows, 0, numbers + 1, top, bottom)
    if number_y is None:
        return origin
    x = origin
    while x < origin + 8:
        y = first_ink_row(rows, x, x + 1, top, bottom)
        if y is None or y + 1 >= number_y:
            return x
        x += 1
    return origin


def check(name, rows, top, bottom, origin, numbers):
    if stray_ink(rows, top, bottom, numbers + 2, origin):
        raise SystemExit(f"{name}: text ink in the margin")
    if first_ink_row(rows, 0, numbers + 1, top, bottom) is None:
        raise SystemExit(f"{name}: line numbers are not visible")


def is_marker(pixel, background):
    if dist(pixel, background) <= 18:
        return False
    red, green, blue = pixel
    added = green > red + 35 and green > blue + 15 and green > 80
    modified = blue > red + 35 and blue > green + 20 and blue > 80
    removed = red > green + 40 and red > blue + 40 and red > 100
    return added or modified or removed


def editor_strip(rows):
    """Gutter background and the rows that are the editor, not the chrome."""
    height = len(rows)
    column = [rows[y][1] for y in range(height)]
    sample = column[height // 5 : 4 * height // 5]
    background = max(set(sample), key=sample.count)
    top = next(y for y in range(height) if dist(rows[y][1], background) <= 8)
    bottom = height
    for y in range(height - 1, top, -1):
        if dist(rows[y][1], background) <= 8:
            bottom = y + 1
            break
    return background, top, bottom


def assert_digit_margin(name, rows):
    """Leftmost digit column has >= 1px of gutter, and nothing covers a glyph.

    The settings-palette shots use `fn theme_probe` / `let n = 1;`. A new
    file in the repo paints a change marker beside the numbers. The marker
    used to start inside the units digit, so "10" read as "1(" and a single
    digit was cut in half.
    """
    background, top, bottom = editor_strip(rows)
    width = len(rows[0])
    limit = min(width, 180)
    digit_cols = []
    for x in range(limit):
        digits = 0
        marks = 0
        for y in range(top, bottom):
            pixel = rows[y][x]
            if dist(pixel, background) <= 18:
                continue
            if is_marker(pixel, background):
                marks += 1
            else:
                digits += 1
        if digits >= 8 and digits > marks:
            digit_cols.append(x)
    if not digit_cols:
        raise SystemExit(f"{name}: line numbers are not visible")
    left = digit_cols[0]
    right = left
    for x in digit_cols:
        if x - right > 8:
            break
        right = x
    if left < 1:
        raise SystemExit(f"{name}: digit column starts at x={left}, want a margin of at least 1px")
    # Only the rows that hold a digit. A chrome rule above the first line
    # is not the digit column.
    for y in range(top, bottom):
        on_digit = False
        for x in range(left, right + 1):
            pixel = rows[y][x]
            if dist(pixel, background) > 18 and not is_marker(pixel, background):
                on_digit = True
                break
        if not on_digit:
            continue
        # A chrome rule crosses the whole strip. A digit does not.
        span = sum(
            1
            for x in range(min(90, len(rows[0])))
            if dist(rows[y][x], background) > 18 and not is_marker(rows[y][x], background)
        )
        if span > 40:
            continue
        for x in range(left):
            pixel = rows[y][x]
            if dist(pixel, background) > 18 and not is_marker(pixel, background):
                raise SystemExit(f"{name}: digit row y={y} has ink at x={x}")
    gap = None
    for x in range(right + 1, min(width, right + 80)):
        for y in range(top, bottom, 2):
            if dist(rows[y][x], background) > 18:
                gap = x - right - 1
                break
        if gap is not None:
            break
    if gap is None:
        raise SystemExit(f"{name}: no gutter edge after the digits")
    if gap < 1:
        raise SystemExit(
            f"{name}: digits clipped at x={right}, next ink is {gap}px away (want >= 1)"
        )
    print(f"{name}: digit margin x={left}..{right} gap={gap}px")


def load_any_png(path: Path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = None
    idat = b""
    color = depth = None
    while pos + 8 <= len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height = struct.unpack(">II", chunk[:8])
            depth, color = chunk[8], chunk[9]
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
    if color != 6 or depth != 8:
        raise SystemExit(f"{path} is not 8-bit RGBA")
    raw = zlib.decompress(idat)
    bpp = 4
    stride = width * bpp
    rows = []
    prev = bytearray(stride)
    index = 0

    def paeth(left, up, corner):
        estimate = left + up - corner
        la, ua, ca = abs(estimate - left), abs(estimate - up), abs(estimate - corner)
        if la <= ua and la <= ca:
            return left
        if ua <= ca:
            return up
        return corner

    for _ in range(height):
        filt = raw[index]
        index += 1
        row = bytearray(raw[index : index + stride])
        index += stride
        if filt == 1:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + left) & 255
        elif filt == 2:
            for x in range(stride):
                row[x] = (row[x] + prev[x]) & 255
        elif filt == 3:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + ((left + prev[x]) // 2)) & 255
        elif filt == 4:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                up = prev[x]
                corner = prev[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + paeth(left, up, corner)) & 255
        elif filt != 0:
            raise SystemExit(f"{path} uses PNG filter {filt}")
        prev = row
        rows.append([tuple(row[x * 4 : x * 4 + 3]) for x in range(width)])
    return rows


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "--margin":
        for name in sys.argv[2:]:
            assert_digit_margin(name, load_any_png(Path(name)))
        print("digit margin ok")
        return
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
    before_x = text_body_x(before, origin, top, bottom, numbers)
    after_x = text_body_x(vertical, origin, top, bottom, numbers)
    before_delta = first_ink_row(before, before_x, before_x + 30, top, bottom) - first_ink_row(
        before, 0, numbers + 1, top, bottom
    )
    after_delta = first_ink_row(vertical, after_x, after_x + 30, top, bottom) - first_ink_row(
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
    for name, rows in (
        ("01-before.png", before),
        ("02-after-shift-wheel.png", shifted),
        ("03-after-delta-x.png", delta_x),
        ("04-after-vertical.png", vertical),
        ("05-wrap-on.png", wrapped),
    ):
        assert_digit_margin(name, rows)
    print("gutter pixels ok")


if __name__ == "__main__":
    main()
