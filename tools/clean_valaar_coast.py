#!/usr/bin/env python3
"""Strip coastal valaar: any surface `valaar` cell that orthogonally touches an
`ocean` cell becomes `land`. Cleans the rasterizer's water-edge artifacts where
rivers/reservoir bleed against the coast.

Operates in place on a TerrainMap JSON ({w,h,layers,seed,cells}); only the
surface plane (first w*h cells) is touched. Uses a snapshot so it's a single
pass (valaar->land never cascades).

Usage: python3 tools/clean_valaar_coast.py [map.json]   (default out/alchaea.json)
"""
import json
import sys

OCEAN, LAND, VALAAR = "ocean", "land", "valaar"


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "out/alchaea.json"
    doc = json.load(open(path))
    w, h, cells = doc["w"], doc["h"], doc["cells"]
    surface = cells[: w * h]  # snapshot via slice (separate list)

    changed = 0
    for y in range(h):
        for x in range(w):
            i = y * w + x
            if surface[i] != VALAAR:
                continue
            touches_ocean = (
                (x > 0 and surface[i - 1] == OCEAN)
                or (x + 1 < w and surface[i + 1] == OCEAN)
                or (y > 0 and surface[i - w] == OCEAN)
                or (y + 1 < h and surface[i + w] == OCEAN)
            )
            if touches_ocean:
                cells[i] = LAND
                changed += 1

    with open(path, "w") as f:
        json.dump(doc, f)
    print(f"{path}: {changed} coastal valaar cell(s) -> land")


if __name__ == "__main__":
    main()
