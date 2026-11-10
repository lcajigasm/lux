#!/usr/bin/env python3
"""Pixel checks for the theme-break captures.

Reads the PNGs written by scripts/theme-break.sh. Lux snapshots are
8-bit RGBA with filter 0. screencapture may use the other PNG filters.
"""

import json
import sys
import zlib
from pathlib import Path


def parse_hex(text):
    raw = text.strip().lstrip("#")
    if len(raw) == 8:
        raw = raw[:6]
    if len(raw) != 6:
        raise SystemExit(f"bad color {text}")
    return tuple(int(raw[i : i + 2], 16) for i in (0, 2, 4))


def hex_of(rgb):
    return "".join(f"{c:02X}" for c in rgb)


def shift(rgb, delta):
    return tuple(max(0, min(255, c + delta)) for c in rgb)


def derived(background):
    return {
        "bg": background,
        "tab": shift(background, -16),
        "status": shift(background, -16),
        "panel": shift(background, -12),
        "row": shift(background, 32),
        "input": shift(background, 18),
    }


def optional(globals_, key, fallback):
    value = globals_.get(key)
    if isinstance(value, str) and value.startswith("#"):
        return parse_hex(value)
    return fallback


BUILTINS = {
    "monokai": {
        "bg": parse_hex("272822"),
        "tab": parse_hex("1E1F1C"),
        "status": parse_hex("414339"),
        "panel": parse_hex("1E1F1C"),
        "row": parse_hex("49483E"),
        "input": parse_hex("414339"),
    },
    "mariana": derived(parse_hex("303841")),
    "breakers": derived(parse_hex("0C1014")),
    "celeste": derived(parse_hex("F8F9FA")),
    "lux-dark": derived(parse_hex("12121B")),
    "lux-light": derived(parse_hex("FAFAFD")),
    "high-contrast": derived(parse_hex("000000")),
}


def normalize(name):
    return name.strip().lower().replace("-", "").replace("_", "").replace(" ", "")


def scheme_row(path):
    data = json.loads(path.read_text())
    name = data.get("name") or path.stem
    globals_ = data.get("globals") or {}
    background = parse_hex(globals_["background"])
    colors = derived(background)
    colors["bg"] = background
    colors["tab"] = optional(globals_, "tab_bar", colors["tab"])
    colors["status"] = optional(globals_, "status_bar", colors["status"])
    colors["panel"] = optional(globals_, "panel", colors["panel"])
    colors["row"] = optional(globals_, "panel_row", colors["row"])
    colors["input"] = optional(globals_, "panel_input", colors["input"])
    return name, colors


def list_themes(themes_dir):
    seen = set()
    rows = [("system", None)]
    for name, colors in BUILTINS.items():
        rows.append((name, colors))
        seen.add(normalize(name))
    for path in sorted(Path(themes_dir).glob("*.sublime-color-scheme")):
        name, colors = scheme_row(path)
        key = normalize(name)
        if key in seen:
            continue
        seen.add(key)
        rows.append((name, colors))
    for name, colors in rows:
        if colors is None:
            print(f"{name}\tany")
            continue
        fields = [
            name,
            hex_of(colors["bg"]),
            hex_of(colors["tab"]),
            hex_of(colors["status"]),
            hex_of(colors["panel"]),
            hex_of(colors["row"]),
            hex_of(colors["input"]),
        ]
        print("\t".join(fields))


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def read_png(path):
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = color = depth = None
    idat = b""
    while pos + 8 <= len(data):
        length = int.from_bytes(data[pos : pos + 4], "big")
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height = int.from_bytes(chunk[:4], "big"), int.from_bytes(chunk[4:8], "big")
            depth, color = chunk[8], chunk[9]
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
    if color not in (2, 6) or depth != 8:
        raise SystemExit(f"{path} is color {color} depth {depth}, want 8-bit RGB or RGBA")
    bpp = 4 if color == 6 else 3
    raw = zlib.decompress(idat)
    stride = width * bpp
    out = bytearray(height * stride)
    prev = bytearray(stride)
    index = 0
    for y in range(height):
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
                upper_left = prev[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + paeth(left, up, upper_left)) & 255
        elif filt != 0:
            raise SystemExit(f"{path} row filter {filt}")
        out[y * stride : (y + 1) * stride] = row
        prev = row
    return width, height, bpp, out


class Image:
    def __init__(self, path):
        self.width, self.height, self.bpp, self.raw = read_png(path)
        self.path = path

    def at(self, x, y):
        x = min(max(x, 0), self.width - 1)
        y = min(max(y, 0), self.height - 1)
        i = (y * self.width + x) * self.bpp
        return self.raw[i], self.raw[i + 1], self.raw[i + 2]


def dist(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def near(a, b, tol):
    return dist(a, b) <= tol


def mode(image, x0, x1, y0, y1, step=3):
    counts = {}
    x0, x1 = max(0, x0), min(image.width, x1)
    y0, y1 = max(0, y0), min(image.height, y1)
    for y in range(y0, y1, step):
        for x in range(x0, x1, step):
            pixel = image.at(x, y)
            key = tuple(c & ~7 for c in pixel)
            counts[key] = counts.get(key, 0) + 1
    if not counts:
        raise SystemExit(f"empty sample in {image.path}")
    return max(counts, key=counts.get)


def lin(channel):
    c = channel / 255.0
    if c <= 0.04045:
        return c / 12.92
    return ((c + 0.055) / 1.055) ** 2.4


def luminance(rgb):
    return 0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])


def contrast(a, b):
    left, right = luminance(a), luminance(b)
    lighter, darker = max(left, right), min(left, right)
    return (lighter + 0.05) / (darker + 0.05)


def fail(message):
    print(f"FAIL {message}")
    raise SystemExit(1)


def check_editor(path, background, tab, status):
    image = Image(path)
    bg = parse_hex(background)
    tab_c = parse_hex(tab)
    status_c = parse_hex(status)
    width, height = image.width, image.height
    if width < 800 or height < 500:
        fail(f"{path} is {width}x{height}")
    body = mode(image, int(width * 0.18), int(width * 0.72), 90, height - 70)
    if not near(body, bg, 24):
        fail(f"{path} editor {hex_of(body)} wanted {background}")
    ink_rows = []
    for y in range(72, height - 56, 2):
        for x in range(int(width * 0.14), int(width * 0.68), 3):
            if dist(image.at(x, y), body) > 48:
                ink_rows.append(y)
                break
    if len(ink_rows) < 8:
        fail(f"{path} has almost no editor text ({len(ink_rows)} rows)")
    span = ink_rows[-1] - ink_rows[0]
    if span < 180:
        fail(f"{path} text spans only {span}px; lines look collapsed")
    tab_mode = mode(image, 180, width // 2, 6, 52, 2)
    if not near(tab_mode, tab_c, 28):
        fail(f"{path} tab bar {hex_of(tab_mode)} wanted {tab}")
    status_mode = mode(image, 180, width // 2, height - 40, height - 6, 2)
    if not near(status_mode, status_c, 28):
        fail(f"{path} status bar {hex_of(status_mode)} wanted {status}")
    marks = 0
    for y in range(80, min(height - 60, 520), 2):
        for x in range(width - 160, width - 12, 2):
            if dist(image.at(x, y), body) > 40:
                marks += 1
    if marks < 12:
        fail(f"{path} minimap has {marks} ink pixels")
    edge = mode(image, width - 3, width, 80, height - 60, 2)
    inside = mode(image, width - 28, width - 14, 80, height - 60, 2)
    if not near(edge, inside, 36):
        fail(f"{path} right edge {hex_of(edge)} differs from {hex_of(inside)}")
    print(
        f"PASS editor {path} {width}x{height} bg={hex_of(body)} "
        f"text_span={span}px minimap_ink={marks}"
    )


def check_editor_loose(path):
    """System theme follows the OS, so the exact hex is not asserted."""
    image = Image(path)
    width, height = image.width, image.height
    body = mode(image, int(width * 0.18), int(width * 0.72), 90, height - 70)
    ink_rows = []
    for y in range(72, height - 56, 2):
        for x in range(int(width * 0.14), int(width * 0.68), 3):
            if dist(image.at(x, y), body) > 48:
                ink_rows.append(y)
                break
    if len(ink_rows) < 8 or ink_rows[-1] - ink_rows[0] < 180:
        fail(f"{path} system theme did not paint a multi-line buffer")
    edge = mode(image, width - 3, width, 80, height - 60, 2)
    inside = mode(image, width - 28, width - 14, 80, height - 60, 2)
    if not near(edge, inside, 36):
        fail(f"{path} paints past the window edge")
    print(f"PASS editor {path} system bg={hex_of(body)}")


def check_palette(path, row_fill):
    image = Image(path)
    width, height = image.width, image.height
    card_w = width * 0.46
    left = (width - card_w) * 0.5
    top = 64 + 16
    row_y = top + 92 + 4
    row = mode(
        image,
        int(left + 70),
        int(left + card_w - 40),
        int(row_y + 8),
        int(row_y + 64),
        2,
    )
    card = mode(image, int(left + 40), int(left + card_w - 40), int(top + 2), int(top + 12), 2)
    # The selection is pushed to 3:1 against the card, so it is no longer
    # the raw panel-row hex. `row_fill` stays in the call for the job log.
    _ = row_fill
    row_ratio = contrast(row, card)
    if row_ratio < 2.9:
        fail(
            f"{path} palette selection {hex_of(row)} on card {hex_of(card)} "
            f"is {row_ratio:.2f}, want >= 2.9 ({row_fill})"
        )
    if not near(card, parse_hex("75715E"), 12) and near(row, parse_hex("75715E"), 18):
        fail(f"{path} palette row is the old beige {hex_of(row)}")
    inks = []
    heights = []
    title_x0 = int(left + 16 + 32 + 20)
    title_x1 = int(left + card_w * 0.55)
    for x in range(title_x0, title_x1):
        column = []
        for y in range(int(row_y + 6), int(row_y + 66)):
            pixel = image.at(x, y)
            if dist(pixel, row) > 70:
                column.append((y, pixel))
        if len(column) >= 2:
            heights.append(column[-1][0] - column[0][0] + 1)
            inks.extend(pixel for _, pixel in column)
    if len(heights) < 4:
        fail(f"{path} palette title has no readable glyphs")
    heights.sort()
    glyph_h = heights[len(heights) // 2]
    if glyph_h < 12:
        fail(f"{path} palette glyphs are {glyph_h}px at scale 2")
    inks.sort(key=luminance)
    # Glyph cores, not the antialiased edge. A light row uses the darkest
    # stems; a dark row uses the lightest.
    count = max(1, len(inks) // 20)
    core = inks[:count] if luminance(row) > 0.4 else inks[-count:]
    ink = core[len(core) // 2]
    ratio = contrast(ink, row)
    if ratio < 4.5:
        fail(f"{path} palette contrast {ratio:.2f} ({hex_of(ink)} on {hex_of(row)})")
    pink = parse_hex("F92672")
    pink_px = 0
    total = 0
    for y in range(int(row_y + 16), int(row_y + 52), 2):
        for x in range(int(left + 28), int(left + 28 + 36), 2):
            total += 1
            if dist(image.at(x, y), pink) < 28:
                pink_px += 1
    if total and pink_px / total > 0.2:
        fail(f"{path} palette icon is a pink block ({pink_px}/{total})")
    print(
        f"PASS palette {path} row={hex_of(row)} contrast={ratio:.2f} glyph={glyph_h}px"
    )


def check_settings(path, panel):
    image = Image(path)
    width, height = image.width, image.height
    expected = parse_hex(panel)
    body = mode(image, 460, min(width - 40, 1200), 280, height - 80)
    if not near(body, expected, 30):
        fail(f"{path} settings page {hex_of(body)} wanted {panel}")
    sidebar_ink = 0
    for y in range(200, 460, 3):
        for x in range(24, 340, 3):
            if dist(image.at(x, y), body) > 28:
                sidebar_ink += 1
    if sidebar_ink < 20:
        fail(f"{path} settings sidebar has no labels ({sidebar_ink})")
    search = mode(image, 460, 900, 180, 230, 2)
    if near(search, body, 10):
        fail(f"{path} settings search field matches the page fill")
    # settings/layout.rs places the thumb at
    # page.right - (PAD + 8) * scale, PAD = 28, snapshot scale 2:
    # about 72px in from the right, 8px wide. The window edge is page fill.
    thumb = 0
    for y in range(240, height - 40, 2):
        for x in range(width - 84, width - 56):
            if dist(image.at(x, y), body) > 22:
                thumb += 1
    if thumb < 20:
        fail(f"{path} settings has no scrollbar ({thumb})")
    print(f"PASS settings {path} page={hex_of(body)} sidebar_ink={sidebar_ink} scroll={thumb}")


def write_png(path, width, height, rgb):
    raw = bytearray()
    stride = width * 3
    for y in range(height):
        raw.append(0)
        raw.extend(rgb[y * stride : (y + 1) * stride])
    compressed = zlib.compress(bytes(raw), 9)
    out = bytearray(b"\x89PNG\r\n\x1a\n")

    def chunk(kind, payload):
        out.extend(len(payload).to_bytes(4, "big"))
        out.extend(kind)
        out.extend(payload)
        out.extend(zlib.crc32(kind + payload).to_bytes(4, "big"))

    ihdr = width.to_bytes(4, "big") + height.to_bytes(4, "big") + bytes([8, 2, 0, 0, 0])
    chunk(b"IHDR", ihdr)
    chunk(b"IDAT", compressed)
    chunk(b"IEND", b"")
    Path(path).write_bytes(out)


def composite(dest, left, middle, right):
    images = [Image(left), Image(middle), Image(right)]
    target_h = 720
    gap = 16
    resized = []
    for image in images:
        scale = target_h / image.height
        w = max(1, int(image.width * scale))
        buf = bytearray(w * target_h * 3)
        for y in range(target_h):
            sy = min(image.height - 1, int(y / scale))
            for x in range(w):
                sx = min(image.width - 1, int(x / scale))
                pixel = image.at(sx, sy)
                i = (y * w + x) * 3
                buf[i : i + 3] = bytes(pixel)
        resized.append((w, buf))
    width = sum(w for w, _ in resized) + gap * 2
    canvas = bytearray([32, 32, 32]) * (width * target_h)
    x_at = 0
    for w, buf in resized:
        for y in range(target_h):
            dst = (y * width + x_at) * 3
            src = y * w * 3
            canvas[dst : dst + w * 3] = buf[src : src + w * 3]
        x_at += w + gap
    write_png(dest, width, target_h, canvas)
    print(f"PASS composite {dest} {width}x{target_h}")


def main(argv):
    if len(argv) < 2:
        raise SystemExit("usage: theme_pixels.py list|editor|palette|settings|composite ...")
    cmd = argv[1]
    if cmd == "list":
        list_themes(argv[2])
    elif cmd == "editor":
        if argv[3] == "any":
            check_editor_loose(argv[2])
        else:
            check_editor(argv[2], argv[3], argv[4], argv[5])
    elif cmd == "palette":
        check_palette(argv[2], argv[3])
    elif cmd == "settings":
        check_settings(argv[2], argv[3])
    elif cmd == "composite":
        composite(argv[2], argv[3], argv[4], argv[5])
    else:
        raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    main(sys.argv)
