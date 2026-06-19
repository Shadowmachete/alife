# Tile design guide (plan 4 — static geography & map visualiser)

> A practical, opinionated guide for authoring the tile art that the plan-4 map
> visualiser will render. Goal: tiles that (a) read clearly as a top-down map,
> (b) match the Alchaea lore, and (c) drop into the engine without renaming. The
> engine is the source of truth: a tile = one `CellType`. Author files named to
> match the enum below and the renderer is a trivial `CellType → PNG` lookup.

---

## 1. The canonical tile set (name your files to match)

Plan 4's terrain map stores, per cell: **`cell_type`** (which tile) + **valaar
conductivity** (how freely valaar diffuses through it) + **passability** (can an
organism move into it). Conductivity/passability live in code; you only draw the
**cell_type**. Recommended starting set (the engine `CellType` enum will mirror
these names — file = `assets/tiles/<snake_case>.png`):

| Tile (file)        | Lore role                              | Conductivity | Passable? | Notes for the art |
|--------------------|----------------------------------------|--------------|-----------|-------------------|
| `rasconne.png`     | The central volcano / valaar reservoir | source (max) | yes       | The hero tile. Hot core, glowing caldera. |
| `river.png`        | Valaar river corridor (lush)           | high         | yes       | Glowing channel — the "lush" path life follows. |
| `land.png`         | Continent surface / plains (default)   | normal       | yes       | The neutral baseline; most of the map. |
| `rock.png`         | Barren / Dusk periphery                | low          | yes       | Drab, desaturated — valaar barely flows. |
| `mountain.png`     | Mountain range                         | none (block) | no / slow | Barrier *within* a continent. |
| `ocean.png`        | Ocean                                  | none (block) | no        | The hard barrier between continents (bridged only at Vraze). |
| `coast.png` *(opt)*| Land/ocean transition                  | low          | yes       | Optional; smooths the shoreline. |
| `cave.png` *(opt)* | Access point to the underground layer  | special      | yes       | Optional now — for the 2nd (Underground) layer later. |

**Minimum viable set to unblock the visualiser:** `ocean`, `land`, `river`,
`mountain`, `rasconne`. Add `rock`, `coast`, `cave` when you want them. If you
want different names/splits, tell me and I'll match the enum to your art — but
**lock the set before drawing a lot**, so we don't rename later.

> Why these and not "Dusk" as a tile: the Dusk is *emergent* — it's wherever the
> diffusing valaar field has thinned out (far from sources), not a painted
> terrain. `rock` is for genuinely barren ground; the Dusk gradient is drawn by
> the valaar overlay (below), not by a tile.

---

## 2. Format & dimensions

- **Top-down orthogonal view** (not isometric). The world is a plain `(x, y)`
  grid (`Grid2p5D`), rendered row-by-row like the existing `examples/heatmap.rs`.
  Isometric would fight the data model.
- **Square, power-of-two: 32×32 px.** Big enough for a few shades of detail,
  small enough that a 60×30-ish map fits on screen and scales crisply by integer
  factors (×2, ×3). 16×16 is fine if you want a denser map; pick one and keep
  **every tile identical in size**.
- **PNG, RGBA** (transparency only matters for overlays/props; base terrain can
  be fully opaque).
- **Tileable edges:** a tile sits edge-to-edge with copies of itself, so avoid
  features that only make sense at one position. In GIMP/Krita use
  *Layer → Transform → Offset* (wrap-around) to check the seams; in Aseprite the
  tilemap mode previews adjacency.
- **Consistent light direction** across all tiles (e.g. top-left), or the map
  looks like patchwork.
- **2–3 variants of the common tiles** (`land`, `ocean`, `rock`) break up visible
  repetition. Name them `land.png`, `land_1.png`, `land_2.png`; the renderer can
  pick one deterministically from the cell coordinate.

**Delivery:** simplest is **one PNG per tile** in `assets/tiles/`. (A single
packed tilesheet + atlas is more scalable but adds a coordinate-bookkeeping step
— not worth it at this size. If you'd rather do a sheet, do a fixed grid, e.g.
all tiles in one row, and tell me the order.)

---

## 3. Style guide (palette tied to the lore)

The world's signature is **valaar**: bioluminescent mana. Lean into a "glow vs.
drab" contrast — valaar-rich places glow (teal→cyan→violet), the Dusk/barren is
desaturated grey-violet, and Rasconne is hot (orange→magenta).

- **Use a fixed, small palette** (~12–24 colours) so tiles feel like one set.
  Build or grab one from **Lospec** (see §4). A loose suggestion:
  - Rasconne / heat: deep red → orange → hot magenta highlight.
  - Valaar (river/glow): teal `#1b9e8f`-ish → cyan → pale violet highlight.
  - Land: muted green/olive (alien, slightly desaturated — "prehistoric").
  - Rock / Dusk: grey with a cold violet tint, low saturation.
  - Ocean: dark indigo/teal, *not* bright blue — keep it ominous (it's a barrier).
  - Mountain: cool grey with darker crevices.
- **Value first, hue second:** make the tiles read correctly in greyscale (ocean
  darkest, rasconne brightest). That guarantees the map is legible even before
  colour does its job.
- **Don't bake lighting/shadows from neighbours into the tile** — keep each tile
  self-contained so any tile can sit next to any other.
- **Season tinting is the renderer's job, not yours.** Don't draw "summer land"
  vs "winter land"; draw neutral tiles and the visualiser can tint by season
  (Rasgun glow, Dansch drought-yellow, etc.) as a later overlay. Author for the
  neutral/Goscon baseline.

---

## 4. Tools, repos & references

**Editors (pick one):**
- **Aseprite** (~$20, or free if you build from source) — *the* pixel-art/tile
  tool. Has a dedicated tilemap mode, palette management, animation. Best choice
  if you'll do much of this.
- **LibreSprite** (free, FOSS fork of old Aseprite) — same workflow, no cost.
- **Piskel** (free, web + desktop, piskelapp.com) — zero-install, great for
  quick square tiles.
- **GIMP / Krita** (free, raster) — fine for tiles; use the offset/wrap tools for
  seamless edges. Krita has nice pixel brushes.

**Map layout / testing (optional but handy):**
- **Tiled** (free, mapeditor.org) — load your tiles as a tileset and paint a test
  map to eyeball how they sit together before the engine renders anything. Also a
  viable way for *you* to hand-author the sketch map (exports JSON/TMX I can read).

**Palettes & learning:**
- **Lospec.com** — palette database (filter by size) + a solid pixel-art tutorial
  library. Start here for a cohesive palette.
- **Slynyrd (slynyrd.com)** — excellent free pixel-art tutorials, incl. top-down
  tiles and terrain.
- *Pixel Logic* (Michael Azzi) — the standard paid pixel-art handbook.

**Reference tilesets (for conventions — how pros handle top-down terrain):**
- **Kenney.nl** — huge CC0 (public-domain) asset packs; his top-down/RTS tile
  sets are a great structural reference even if the style differs.
- **OpenGameArt.org** — CC0/CC-BY tilesets; filter by licence.
- **itch.io** game-assets — many free/cheap top-down tilesets for inspiration.
  (Mind the licence if you ever ship anything derived.)

---

## 5. What-to-do checklist

1. **Lock the tile set** (§1) — confirm names with me so the enum matches.
2. **Pick a palette** on Lospec (~16 colours), tuned to the glow-vs-drab idea.
3. **Set canvas = 32×32**, light from one direction.
4. For each tile: block the base value, add 2–3 shades, keep edges tileable
   (offset-check the seams).
5. **Variants** for `land`/`ocean`/`rock` (×2–3) to kill repetition.
6. **Export** each as `assets/tiles/<name>.png` (32×32, RGBA).
7. Drop a **`legend.md`** (or just tell me) mapping any non-obvious tile → meaning
   so the `CellType` enum, conductivity, and passability line up with your intent.

When you've got even the 5-tile minimum set + the map sketch, hand them over and
I'll wire the procedural generator + visualiser (plan 4) to render them.
