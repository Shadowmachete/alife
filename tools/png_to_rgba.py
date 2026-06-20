#!/usr/bin/env python3
"""Bake a PNG into a raw RGBA blob the Rust viewer reads without a PNG decoder.

Format: header `width:u32_le, height:u32_le`, then `width*height*4` RGBA bytes.
Re-run whenever the atlas art (sheet.png) changes.

Usage: python3 tools/png_to_rgba.py [in.png] [out.rgba]
       defaults: assets/sheet.png  assets/sheet.rgba
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tmx_autotile import decode_png  # reuse the stdlib PNG decoder


def main():
    inp = sys.argv[1] if len(sys.argv) > 1 else "assets/sheet.png"
    out = sys.argv[2] if len(sys.argv) > 2 else "assets/sheet.rgba"
    w, h, ch, px = decode_png(inp)
    with open(out, "wb") as f:
        f.write(struct.pack("<II", w, h))
        if ch == 4:
            f.write(bytes(px))
        else:  # RGB -> RGBA (opaque)
            rgba = bytearray()
            for i in range(0, len(px), 3):
                rgba += px[i:i + 3]
                rgba.append(255)
            f.write(bytes(rgba))
    print(f"{inp} ({w}x{h}, {ch}ch) -> {out} ({8 + w * h * 4} bytes)")


if __name__ == "__main__":
    main()
