#!/usr/bin/env python3
"""Checks for a Lux SIGUSR1 frame.

The encoder in `snapshot.rs` writes 8-bit RGBA with filter 0. The editor
well is Monokai `#272822`.
"""

import struct
import sys
import zlib
from pathlib import Path

WELL = (0x27, 0x28, 0x22)


def load(path: Path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = None
    idat = b""
    while pos + 8 <= len(data):
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
    if width is None:
        raise SystemExit(f"{path} has no IHDR")
    raw = zlib.decompress(idat)
    stride = width * 4
    rows = []
    src = 0
    for _ in range(height):
        if src >= len(raw) or raw[src] != 0:
            raise SystemExit(f"{path} uses a PNG filter")
        src += 1
        rows.append(raw[src : src + stride])
        src += stride
    return width, height, rows


def sample(path: Path):
    width, height, rows = load(path)
    pts = []
    for y in range(0, height, 4):
        row = rows[y]
        for x in range(0, width, 4):
            pts.append(row[x * 4 : x * 4 + 3])
    return width, height, pts


def near_well(pixel: bytes) -> bool:
    return (
        abs(pixel[0] - WELL[0]) <= 8
        and abs(pixel[1] - WELL[1]) <= 8
        and abs(pixel[2] - WELL[2]) <= 8
    )


def require_frame(path: Path) -> None:
    width, height, pts = sample(path)
    if width < 400 or height < 300:
        raise SystemExit(f"{path} is {width}x{height}")
    wells = sum(1 for px in pts if near_well(px))
    if wells < 40:
        raise SystemExit(f"{path} has no editor well ({wells})")
    print(f"{path} {width}x{height} well {wells}")


def require_ink(path: Path) -> None:
    _, _, pts = sample(path)
    ink = sum(1 for px in pts if not near_well(px))
    if ink < 30:
        raise SystemExit(f"{path} has no text ink ({ink})")
    print(f"{path} ink {ink}")


def require_differ(left: Path, right: Path) -> None:
    lw, lh, lp = sample(left)
    rw, rh, rp = sample(right)
    if (lw, lh) != (rw, rh):
        raise SystemExit(f"size {lw}x{lh} vs {rw}x{rh}")
    changed = sum(1 for a, b in zip(lp, rp) if a != b)
    if changed < 200:
        raise SystemExit(f"frames match ({changed} sampled pixels differ)")
    print(f"differ {changed}")


def main() -> None:
    cmd = sys.argv[1]
    if cmd == "frame":
        require_frame(Path(sys.argv[2]))
    elif cmd == "ink":
        require_ink(Path(sys.argv[2]))
    elif cmd == "differ":
        require_differ(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        raise SystemExit(f"unknown {cmd}")


if __name__ == "__main__":
    main()
