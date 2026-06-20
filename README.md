# alife — emergent natural history on a custom world

An artificial-life engine that grows a believable **biosphere** on a hand-designed
world and, as a by-product, produces **worldbuilding lore**: emergent species,
biomes, food webs, and a timeline of radiations and extinctions.

The guiding principle: **the world's physics *are* the selection pressure.**
Nothing scores fitness — who survives is decided by traits versus world. Life
genuinely evolves, and the natural history that falls out is the payoff.

**Live demo:** [shadowmachete.github.io/alife](https://shadowmachete.github.io/alife/) —
the `mapview` viewer compiled to WASM (drag to pan, scroll to zoom).

## The world — Alchaea

A prehistoric, alien world born from the hyper-eruption of the supervolcano
**Rasconne**, which shattered the supercontinent Alchaea into drifting continents
and spewed **valaar** — the world's single energy currency ("food" and "power"
are the same resource).

- **Valaar** is a diffusing scalar field: injected at the central **Rasconne**
  reservoir, flowing out along river corridors, thinning with distance. The
  centre-rich → edge-poor gradient is *emergent*, not painted. Organisms eat,
  move, and reproduce by spending valaar; death returns it as detritus.
- **Zones, centre → edge:** Rasconne core → river corridors (lush) → the **Dusk**
  (oligotrophic periphery) → the **Underground** (deep reservoirs, a second layer).
- **The 6-arh year** (702 craws): Rasgun (surge) → Goscon (stable) → Miscre
  (monsoon/fog) → Vraze (quakes/crystalline) → Dansch (drought) → Laisp (lowest
  valaar). Each season rewards a *different* adaptation — the edge-of-chaos engine.
- **Every environmental axis has a matching gene** (you can't adapt to what you
  can't sense): valaar↔`valaar_efficiency`, heat↔`heat_tolerance`,
  water↔`drought_tolerance`. A `swim` gene lets life **tunnel straight through** a
  Valaar river to the far bank (Valaar is impassable to everyone else), paying
  `valaar_drain` per cell crossed — purely Darwinian, the gene alone decides.

Full design rationale lives in `docs/` and the idea notes; the engine constraints
it satisfies are the ones that make genuine emergence possible (spatial
heterogeneity, trade-offs, isolation→speciation, trophic levels, disturbance).

## Status

| Plan | What | State |
|------|------|-------|
| 1 | World substrate — 2.5D space (`Surface`/`Underground` layers) + diffusing valaar field | ✅ done |
| 2 | Organisms + ecology — trait-vector genome, energy economy, implicit selection | ✅ done |
| 3 | Climate + the 6-arh seasons — heat/water fields, tolerance genes, seasonal stress | ✅ done |
| 4 | Static geography — terrain map (cell type + valaar conductivity + passability), sketch-driven generator, interactive map viewer | ✅ done |
| 5+ | Terrain-aware ecology (oceans as barriers → speciation), dynamic geography, the valaar state-machine, the natural-history/timeline layer | 🔮 future |

Plans live in `docs/plans/`. Organisms sit behind a clade-agnostic `Organism`
trait (`TraitOrganism` is the first clade), so richer models (neural brains,
developmental morphology) can drop in later as *new clades*, not rewrites.

## Design principles

- **Implicit selection only** — death and birth are consequences, never a score.
- **Valaar is the one currency** — heat/water are *stress axes* that cost energy,
  never food.
- **Determinism** — identical seed + params ⇒ identical run. One `Rng`, processed
  in a fixed order; no wall-clock or thread nondeterminism.
- **`std`-only core** — the simulation engine uses no external crates. The map
  viewer and map I/O are the only places that pull in dependencies (`eframe`/`egui`
  for the interactive window, `serde`/`serde_json` for map files), kept out of the
  hot path. The viewer is written as an `eframe::App` so it can later build to
  WASM and embed on the web.
- **Spatial access stays behind the `Space` trait** — the world is 2D today
  (`Grid2p5D`); a future `Grid3D` should require no changes elsewhere.

## Running it

```sh
# Tests (unit + integration) and lints
cargo test
cargo clippy --all-targets

# Headless life census — watch selection happen over a couple of years.
# Prints population + mean genes per season; bodies shrink, tolerances rise.
cargo run --bin life

# Single ecology run
cargo run --bin sim

# ASCII valaar heatmap (surface + underground)
cargo run --example heatmap
```

Static geography (plan 4):

```sh
# Rasterize a hand-drawn map sketch (PNG) into a terrain map (out/alchaea.json).
# Colours: blue=ocean, green=land, red=valaar reservoir, black lines=valaar rivers.
python3 tools/sketch_to_map.py [sketch.png] [out.json]

# Author a map in Tiled with the connecting tileset, then upscale + fill in the
# blend tiles (flat regions -> organic coast/riverbanks):
python3 tools/tmx_autotile.py    # assets/alife_map.tmx -> assets/alife_map_blended.tmx
python3 tools/png_to_rgba.py     # bake assets/sheet.png -> assets/sheet.rgba (atlas)

# Open the interactive map viewer (egui): drag to pan, scroll to zoom.
#   .tmx arg  -> textured live sim (default alife_map_blended.tmx). Organisms are
#                circles sized by body, coloured by diet (green autotroph -> red
#                predator), and stay off ocean/valaar. Right panel: population,
#                diet split, mean size, per-continent counts, and Pause/Reseed.
#   .json arg -> TerrainMap in solid CellType colours (layer toggle in the panel)
cargo run --bin mapview [map.tmx | map.json]
```

Build the web demo locally (needs [`trunk`](https://trunkrs.dev) + the
`wasm32-unknown-unknown` target). On wasm the default map + atlas are embedded in
the binary, so no files are read at runtime:

```sh
trunk serve --release                          # http://localhost:8080
trunk build --release --public-url /alife/     # -> dist/ (what CI deploys to Pages)
```

## Layout

```
src/
  space.rs        2.5D spatial model (Coord, Layer, Space trait, Grid2p5D)
  field.rs        scalar field (one f32 per cell)
  valaar.rs       valaar dynamics (inject / diffuse / layer-exchange / decay)
  world.rs        World = Space + valaar field + sources/access points
  rng.rs          deterministic PRNG
  genome.rs       trait-vector genome (9 traits, incl. swim)
  organism.rs     Organism trait + TraitOrganism (first clade)
  population.rs    organism store + occupancy index
  params.rs       EcoParams — every ecology rate as a dial
  ecology.rs      tick functions (absorb / move / predate / stress / metabolize / cull / reproduce)
  season.rs       the 6-arh calendar
  climate.rs      season → heat/water targets + field relaxation
  sim.rs          Sim — weaves world + climate + ecology into one tick
  terrain.rs      (plan 4) CellType + TerrainMap + JSON I/O
  viewer.rs       (plan 4) pan/zoom camera + solid-colour renderer
  tilemap.rs      (plan 4) Tiled .tmx + atlas loader + textured renderer
  bin/            life, sim, mapview
tools/
  sketch_to_map.py    rasterize a hand-drawn PNG sketch → terrain map JSON
  tmx_autotile.py     upscale a flat Tiled map + fill in connecting tiles
  png_to_rgba.py      bake a PNG atlas → raw RGBA for the viewer
docs/
  plans/          implementation plans (one per plan)
  tile-design-guide.md
```

## License

Released under the [MIT License](LICENSE) — do what you like with it; keep the
copyright and licence notice in copies.
