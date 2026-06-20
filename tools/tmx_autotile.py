#!/usr/bin/env python3
"""Upscale a flat (hard-edged) Tiled map and fill in the connecting/blend tiles.

The hand-painted map uses one base tile per material (grass / valaar / ocean)
with hard boundaries. This tool:
  1. parses the (infinite, chunked) TMX into a material grid,
  2. upscales it NxN (nearest) so 1-tile rivers gain a core + room to blend,
  3. derives a corner->tile lookup straight from the sheet art (sampling each
     tile's 4 corners), synthesising H/V flips to cover missing orientations,
  4. dual-grid autotiles: each output tile's 4 corners come from the 4
     surrounding upscaled cells, and we place the matching connecting tile,
  5. writes a fresh non-infinite TMX with the tileset embedded inline.

Pure stdlib (xml.etree + zlib). Tiled flip bits live in the GID's top 3 bits.

Usage: python3 tools/tmx_autotile.py [in.tmx] [sheet.png] [out.tmx] [--scale N]
       defaults: assets/alife_map.tmx assets/sheet.png assets/alife_map_blended.tmx --scale 3
"""
import sys
import xml.etree.ElementTree as ET
import zlib
import struct

FLIP_H = 0x80000000
FLIP_V = 0x40000000

# GID -> material for the flat input map (discovered from the export).
GID_MATERIAL = {172: "O", 196: "O", 220: "O", 175: "G", 199: "G", 52: "V"}
# Canonical fill tiles to emit for solid regions (first = preferred).
FILL = {"G": [175, 199], "V": [52], "O": [196, 172, 220]}


# ---------------------------------------------------------------- TMX parsing
def parse_tmx(path):
    root = ET.parse(path).getroot()
    layer = root.find("layer")
    data = layer.find("data")
    chunks = []
    for ch in data.findall("chunk"):
        x, y = int(ch.get("x")), int(ch.get("y"))
        w, h = int(ch.get("width")), int(ch.get("height"))
        nums = [int(v) for v in ch.text.replace("\n", "").split(",") if v.strip()]
        chunks.append((x, y, w, h, nums))
    if not chunks:  # non-infinite fallback
        w, h = int(layer.get("width")), int(layer.get("height"))
        nums = [int(v) for v in data.text.replace("\n", "").split(",") if v.strip()]
        chunks = [(0, 0, w, h, nums)]
    minx = min(c[0] for c in chunks)
    miny = min(c[1] for c in chunks)
    W = max(c[0] + c[2] for c in chunks) - minx
    H = max(c[1] + c[3] for c in chunks) - miny
    grid = [0] * (W * H)
    for x, y, w, h, nums in chunks:
        for i, g in enumerate(nums):
            grid[(y - miny + i // w) * W + (x - minx + i % w)] = g & 0x1FFFFFFF
    return W, H, grid


# ---------------------------------------------------------------- PNG decoding
def decode_png(path):
    d = open(path, "rb").read()
    pos, idat = 8, bytearray()
    while pos < len(d):
        (ln,) = struct.unpack(">I", d[pos:pos + 4])
        ct = d[pos + 4:pos + 8]
        cd = d[pos + 8:pos + 8 + ln]
        pos += 12 + ln
        if ct == b"IHDR":
            w, h, _, col, _, _, _ = struct.unpack(">IIBBBBB", cd)
        elif ct == b"IDAT":
            idat += cd
        elif ct == b"IEND":
            break
    raw = zlib.decompress(bytes(idat))
    ch = 4 if col == 6 else 3
    stride = w * ch
    out = bytearray(h * stride)
    prev = bytearray(stride)
    s = 0
    for y in range(h):
        f = raw[s]
        s += 1
        line = bytearray(raw[s:s + stride])
        s += stride
        if f == 1:
            for i in range(ch, stride):
                line[i] = (line[i] + line[i - ch]) & 255
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif f == 3:
            for i in range(stride):
                a = line[i - ch] if i >= ch else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 255
        elif f == 4:
            for i in range(stride):
                a = line[i - ch] if i >= ch else 0
                c = prev[i - ch] if i >= ch else 0
                b = prev[i]
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
        out[y * stride:(y + 1) * stride] = line
        prev = line
    return w, h, ch, out


# ------------------------------------------------------- corner -> tile lookup
def build_lookup(sheet_path, tile=16):
    sw, sh, sc, px = decode_png(sheet_path)
    cols = sw // tile

    def avg(x0, y0, n):
        rs = gs = bs = cnt = 0
        for yy in range(y0, y0 + n):
            for xx in range(x0, x0 + n):
                o = (yy * sw + xx) * sc
                if sc == 4 and px[o + 3] < 40:
                    continue
                rs += px[o]; gs += px[o + 1]; bs += px[o + 2]; cnt += 1
        return (rs / cnt, gs / cnt, bs / cnt) if cnt else None

    # reference colours from known fill tiles
    def ref(gid):
        loc = gid - 1
        return avg((loc % cols) * tile, (loc // cols) * tile, tile)
    refs = {"G": ref(175), "V": ref(52), "O": ref(196)}

    def classify(c):
        if c is None:
            return None
        return min(refs, key=lambda m: sum((a - b) ** 2 for a, b in zip(c, refs[m])))

    def corners(gid):
        loc = gid - 1
        tx, ty = (loc % cols) * tile, (loc // cols) * tile
        tl = classify(avg(tx + 2, ty + 2, 4))
        tr = classify(avg(tx + 10, ty + 2, 4))
        bl = classify(avg(tx + 2, ty + 10, 4))
        br = classify(avg(tx + 10, ty + 10, 4))
        return (tl, tr, bl, br)

    # transforms: (relabel of (TL,TR,BL,BR), flip bits)
    def xf(sig, kind):
        tl, tr, bl, br = sig
        if kind == "H":
            return (tr, tl, br, bl), FLIP_H
        if kind == "V":
            return (bl, br, tl, tr), FLIP_V
        if kind == "HV":
            return (br, bl, tr, tl), FLIP_H | FLIP_V
        return sig, 0

    lookup = {}
    derived_sigs = set()
    for loc in range(cols * (sh // tile)):
        gid = loc + 1
        sig = corners(gid)
        if None in sig:
            continue                       # transparent/empty tile
        if len(set(sig)) == 1:
            continue                       # solid fill -> handled by FILL below
        if not (set(sig) <= {"G", "V"} or set(sig) <= {"G", "O"}):
            continue                       # never blend valaar<->ocean
        derived_sigs.add(sig)
        for kind in ("", "H", "V", "HV"):
            s2, bits = xf(sig, kind)
            lookup.setdefault(s2, [])
            entry = gid | bits
            if entry not in lookup[s2]:
                lookup[s2].append(entry)
    for m, gids in FILL.items():          # solid-region overrides
        lookup[(m, m, m, m)] = list(gids)
    return lookup, derived_sigs


# ----------------------------------------------------------------- autotiling
def pick(entries, x, y):
    return entries[((x * 73856093) ^ (y * 19349663)) % len(entries)]


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    scale = 3
    for a in sys.argv[1:]:
        if a.startswith("--scale"):
            scale = int(a.split("=")[1] if "=" in a else a.replace("--scale", "").strip() or 3)
    in_tmx = args[0] if len(args) > 0 else "assets/alife_map.tmx"
    sheet = args[1] if len(args) > 1 else "assets/sheet.png"
    out_tmx = args[2] if len(args) > 2 else "assets/alife_map_blended.tmx"

    W, H, grid = parse_tmx(in_tmx)
    mat = ["O"] * (W * H)
    unknown = 0
    for i, g in enumerate(grid):
        m = GID_MATERIAL.get(g)
        if m is None:
            unknown += 1
        mat[i] = m or "O"

    UW, UH = W * scale, H * scale

    def up(x, y):
        if 0 <= x < UW and 0 <= y < UH:
            return mat[(y // scale) * W + (x // scale)]
        return "O"                          # out of bounds = ocean border

    lookup, derived = build_lookup(sheet)

    out = [0] * (UW * UH)
    missing, fills, blends = {}, 0, 0
    for y in range(UH):
        for x in range(UW):
            sig = (up(x - 1, y - 1), up(x, y - 1), up(x - 1, y), up(x, y))
            entries = lookup.get(sig)
            if entries is None:
                key = "".join(sig)
                missing[key] = missing.get(key, 0) + 1
                maj = max(set(sig), key=sig.count)   # fallback: majority fill
                entries = FILL[maj]
            g = pick(entries, x, y)
            if len(set(sig)) == 1:
                fills += 1
            else:
                blends += 1
            out[y * UW + x] = g

    # ---- write TMX (non-infinite, CSV, embedded tileset) ----
    rows = []
    for y in range(UH):
        rows.append(",".join(str(out[y * UW + x]) for x in range(UW)))
    csv = ",\n".join(rows)
    tmx = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<map version="1.10" tiledversion="1.12.2" orientation="orthogonal" '
        f'renderorder="right-down" width="{UW}" height="{UH}" '
        f'tilewidth="16" tileheight="16" infinite="0" nextlayerid="2" nextobjectid="1">\n'
        ' <tileset firstgid="1" name="sheet" tilewidth="16" tileheight="16" '
        'tilecount="288" columns="24">\n'
        '  <image source="sheet.png" width="384" height="192"/>\n'
        ' </tileset>\n'
        f' <layer id="1" name="terrain" width="{UW}" height="{UH}">\n'
        '  <data encoding="csv">\n' + csv + '\n  </data>\n'
        ' </layer>\n'
        '</map>\n'
    )
    open(out_tmx, "w").write(tmx)

    print(f"in  {W}x{H} -> upscaled {UW}x{UH} (x{scale}), {out_tmx}")
    print(f"  unknown input GIDs: {unknown}")
    print(f"  derived blend signatures from sheet: {len(derived)}")
    print(f"  output tiles: {fills} solid, {blends} blended")
    if missing:
        tot = sum(missing.values())
        print(f"  MISSING blend tiles for {len(missing)} signatures ({tot} cells), "
              f"filled with majority: {dict(sorted(missing.items(), key=lambda kv:-kv[1])[:8])}")
    else:
        print("  every required blend had a matching tile ✓")


if __name__ == "__main__":
    main()
