#!/usr/bin/env python3
"""Rasterize a hand-drawn map sketch (PNG) into a TerrainMap JSON for mapview.

Replaces the old sketch->worldgen pipeline: instead of upscaling a coarse Tiled
grid and procedurally stamping features, this traces the drawing's actual shapes.
Each pixel is classified by colour, then downsampled to the map grid:

    blue   -> ocean
    green  -> land
    red    -> valaar   (the Rasconne reservoir)
    black  -> valaar   (a valaar river) -- but ONLY interior black, so the black
                        pen *outlines* around continents stay coast, not river.

Output schema matches terrain.rs `save_json`: {w, h, layers, seed, cells:[...]}
with the surface plane first (row-major) then an all-`rock` underground plane.
CellType serialises lowercase: ocean/land/valaar/rock/mountain.

Usage: python3 tools/sketch_to_map.py [sketch.png] [out.json] [--height N]
       defaults: assets/sketch.png  out/alchaea.json  --height 150
Pure stdlib (zlib only); decodes 8-bit non-interlaced RGB/RGBA PNGs.
"""
import json
import os
import struct
import sys
import zlib

SEED = 0x00A1_C4EA  # the canonical "Alchaea" seed


def read_png(path):
    """Return (width, height, channels, raw_rgb_bytes) for an 8-bit PNG."""
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path}: not a PNG")
    pos, width, height, bitdepth, colortype, interlace = 8, 0, 0, 0, 0, 0
    idat = bytearray()
    while pos < len(data):
        (ln,) = struct.unpack(">I", data[pos:pos + 4])
        ctype = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + ln]
        pos += 12 + ln  # length + type + data + crc
        if ctype == b"IHDR":
            width, height, bitdepth, colortype, _, _, interlace = struct.unpack(
                ">IIBBBBB", chunk
            )
        elif ctype == b"IDAT":
            idat += chunk
        elif ctype == b"IEND":
            break
    if bitdepth != 8 or interlace != 0:
        raise SystemExit(f"{path}: need 8-bit non-interlaced PNG")
    channels = {2: 3, 6: 4}.get(colortype)
    if channels is None:
        raise SystemExit(f"{path}: unsupported colour type {colortype} (need RGB/RGBA)")

    raw = zlib.decompress(bytes(idat))
    stride, bpp = width * channels, channels
    out = bytearray(height * stride)
    prev = bytearray(stride)
    src = 0
    for y in range(height):
        ftype = raw[src]
        src += 1
        line = bytearray(raw[src:src + stride])
        src += stride
        if ftype == 1:  # Sub
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif ftype == 2:  # Up
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:  # Average
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:  # Paeth
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                c = prev[i - bpp] if i >= bpp else 0
                b = prev[i]
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 0xFF
        elif ftype != 0:
            raise SystemExit(f"{path}: bad scanline filter {ftype}")
        out[y * stride:(y + 1) * stride] = line
        prev = line
    return width, height, channels, bytes(out)


def classify(r, g, b):
    """A drawn pixel -> 'K'(black) 'R'(red) 'G'(green/land) 'B'(blue/ocean)."""
    if r < 90 and g < 90 and b < 90:
        return "K"
    if r > 150 and r - g > 40 and r - b > 40:
        return "R"
    if g >= r and g >= b:
        return "G"
    return "B"


def cell_type(counts):
    """Per-cell colour tallies [K,R,G,B] -> CellType string."""
    k, r, g, b = counts
    tot = k + r + g + b
    if tot == 0:
        return "ocean"
    fk, fr, fg, fb = k / tot, r / tot, g / tot, b / tot
    if fr > 0.12:               # the red reservoir
        return "valaar"
    if fk > 0.10 and fg > 0.20 and fb < 0.12:  # interior black line = river
        return "valaar"
    return "land" if fg >= fb else "ocean"


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    height = 150
    for a in sys.argv[1:]:
        if a.startswith("--height="):
            height = int(a.split("=", 1)[1])
    in_path = args[0] if len(args) > 0 else "assets/sketch.png"
    out_path = args[1] if len(args) > 1 else "out/alchaea.json"

    w, h, ch, px = read_png(in_path)
    out_h = height
    out_w = max(1, round(out_h * w / h))
    plane = out_w * out_h
    counts = [[0, 0, 0, 0] for _ in range(plane)]
    idx = {"K": 0, "R": 1, "G": 2, "B": 3}

    for sy in range(h):
        ty = sy * out_h // h
        row = sy * w * ch
        for sx in range(w):
            o = row + sx * ch
            counts[ty * out_w + sx * out_w // w][idx[classify(px[o], px[o + 1], px[o + 2])]] += 1

    surface = [cell_type(c) for c in counts]
    cells = surface + ["rock"] * plane  # underground placeholder, matches engine
    doc = {"w": out_w, "h": out_h, "layers": 2, "seed": SEED, "cells": cells}

    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(doc, f)

    tally = {t: surface.count(t) for t in ("ocean", "land", "valaar")}
    print(f"{in_path} ({w}x{h}) -> {out_path}  map {out_w}x{out_h}")
    print(f"  surface cells: {tally}")


if __name__ == "__main__":
    main()
