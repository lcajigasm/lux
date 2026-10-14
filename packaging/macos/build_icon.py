#!/usr/bin/env python3
"""Builds the high-resolution macOS application icon (lux.icns).

Renders a 1024x1024 master icon adhering to Apple macOS Human Interface
Guidelines (Big Sur / Monterey / Sonoma / Sequoia continuous rounded squircle),
with Lux Dark indigo-black palette (#12121B) and golden amber accent (#F0B442).
Uses macOS `sips` for crisp downsampling and `iconutil` to package `lux.icns`.
"""

import math
import os
import struct
import subprocess
import zlib


def create_icon_1024(out_png_path: str):
    w, h = 1024, 1024
    cx, cy = 512.0, 512.0

    # Apple squircle bounds: 824x824 centered at 512,512 (radius ~185)
    box_hw = 412.0
    box_hh = 412.0
    radius = 185.0

    def sd_rounded_box(px, py, bx, by, bw, bh, r):
        dx = abs(px - bx) - (bw - r)
        dy = abs(py - by) - (bh - r)
        ax = max(dx, 0.0)
        ay = max(dy, 0.0)
        outside = math.hypot(ax, ay) - r
        inside = min(max(dx, dy), 0.0) - r
        return outside if (dx > 0.0 or dy > 0.0) else inside

    raw = bytearray()

    for y in range(h):
        raw.append(0)  # PNG filter: None
        py = float(y)
        for x in range(w):
            px = float(x)

            # Squircle signed distance
            d_sq = sd_rounded_box(px, py, cx, cy, box_hw, box_hh, radius)
            sq_alpha = max(0.0, min(1.0, 0.5 - d_sq))
            if sq_alpha <= 0.0:
                raw.extend([0, 0, 0, 0])
                continue

            # Indigo background gradient
            t_grad = (py - (cy - box_hh)) / (2.0 * box_hh)
            t_grad = max(0.0, min(1.0, t_grad))
            bg_r = int(24.0 * (1.0 - t_grad) + 14.0 * t_grad)
            bg_g = int(24.0 * (1.0 - t_grad) + 14.0 * t_grad)
            bg_b = int(36.0 * (1.0 - t_grad) + 22.0 * t_grad)

            # Subtle top rim lighting
            if d_sq > -6.0 and py < cy:
                rim_t = max(0.0, (6.0 + d_sq) / 6.0)
                bg_r = min(255, int(bg_r + 40.0 * rim_t))
                bg_g = min(255, int(bg_g + 40.0 * rim_t))
                bg_b = min(255, int(bg_b + 55.0 * rim_t))

            # Lux Emblem Geometry: stylized 'L' with diamond spark
            d_vbar = sd_rounded_box(px, py, 395.0, 490.0, 55.0, 220.0, 24.0)
            d_hbar = sd_rounded_box(px, py, 515.0, 685.0, 175.0, 55.0, 24.0)
            d_l = min(d_vbar, d_hbar)

            sp_x = (px - 590.0) * 0.7071 - (py - 390.0) * 0.7071
            sp_y = (px - 590.0) * 0.7071 + (py - 390.0) * 0.7071
            d_spark_rot = sd_rounded_box(sp_x, sp_y, 0.0, 0.0, 50.0, 50.0, 18.0)
            d_emblem = min(d_l, d_spark_rot)

            # Soft drop shadow behind emblem
            d_shadow_v = sd_rounded_box(px, py - 14.0, 395.0, 490.0, 55.0, 220.0, 24.0)
            d_shadow_h = sd_rounded_box(px, py - 14.0, 515.0, 685.0, 175.0, 55.0, 24.0)
            d_shadow_s = sd_rounded_box(sp_x, (py - 14.0 - 390.0) * 0.7071 + (px - 590.0) * 0.7071, 0.0, 0.0, 50.0, 50.0, 18.0)
            d_shadow = min(min(d_shadow_v, d_shadow_h), d_shadow_s)

            if d_shadow < 30.0 and d_emblem > 0.0:
                shadow_alpha = max(0.0, (1.0 - d_shadow / 30.0)) * 0.6
                bg_r = int(bg_r * (1.0 - shadow_alpha * 0.8))
                bg_g = int(bg_g * (1.0 - shadow_alpha * 0.8))
                bg_b = int(bg_b * (1.0 - shadow_alpha * 0.8))

            # Golden amber gradient for emblem
            emblem_alpha = max(0.0, min(1.0, 0.5 - d_emblem))
            if emblem_alpha > 0.0:
                e_t = max(0.0, min(1.0, (py - 270.0) / 480.0))
                if e_t < 0.5:
                    sub_t = e_t / 0.5
                    er = int(255.0 * (1.0 - sub_t) + 240.0 * sub_t)
                    eg = int(226.0 * (1.0 - sub_t) + 180.0 * sub_t)
                    eb = int(115.0 * (1.0 - sub_t) + 66.0 * sub_t)
                else:
                    sub_t = (e_t - 0.5) / 0.5
                    er = int(240.0 * (1.0 - sub_t) + 200.0 * sub_t)
                    eg = int(180.0 * (1.0 - sub_t) + 126.0 * sub_t)
                    eb = int(66.0 * (1.0 - sub_t) + 18.0 * sub_t)

                # Diagonal sheen
                diag = (px - 340.0) - (py - 270.0) * 0.6
                if abs(diag) < 40.0:
                    sheen = max(0.0, 1.0 - abs(diag) / 40.0) * 0.25
                    er = min(255, int(er + 255.0 * sheen))
                    eg = min(255, int(eg + 240.0 * sheen))
                    eb = min(255, int(eb + 200.0 * sheen))

                out_r = int(bg_r * (1.0 - emblem_alpha) + er * emblem_alpha)
                out_g = int(bg_g * (1.0 - emblem_alpha) + eg * emblem_alpha)
                out_b = int(bg_b * (1.0 - emblem_alpha) + eb * emblem_alpha)
            else:
                out_r, out_g, out_b = bg_r, bg_g, bg_b

            final_a = int(255.0 * sq_alpha)
            raw.extend([out_r, out_g, out_b, final_a])

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    png_data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 6)) + chunk(b"IEND", b"")

    os.makedirs(os.path.dirname(out_png_path), exist_ok=True)
    with open(out_png_path, "wb") as f:
        f.write(png_data)


def main():
    root = subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip()
    iconset_dir = os.path.join(root, "packaging", "macos", "Lux.iconset")
    os.makedirs(iconset_dir, exist_ok=True)
    master_png = os.path.join(iconset_dir, "icon_512x512@2x.png")

    print(f"Rendering master 1024x1024 icon to {master_png}...")
    create_icon_1024(master_png)

    # Downscale using sips
    targets = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
    ]

    for fname, size in targets:
        dest = os.path.join(iconset_dir, fname)
        subprocess.check_call(["sips", "-z", str(size), str(size), master_png, "--out", dest], stdout=subprocess.DEVNULL)

    icns_path = os.path.join(root, "packaging", "macos", "lux.icns")
    subprocess.check_call(["iconutil", "-c", "icns", iconset_dir, "-o", icns_path])
    print(f"Generated {icns_path} successfully.")


if __name__ == "__main__":
    main()
