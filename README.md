# alife — emergent natural history on a custom world

An artificial-life engine that grows a believable **biosphere** on a hand-designed
world and, as a by-product, produces **worldbuilding lore**: emergent species,
biomes, food webs, and a timeline of radiations and extinctions.

The guiding principle: **the world's physics *are* the selection pressure.**
Nothing scores fitness — who survives is decided by traits versus world. Life
genuinely evolves, and the natural history that falls out is the payoff.

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
  water↔`drought_tolerance`.

Full design rationale lives in `docs/` and the idea notes; the engine constraints
it satisfies are the ones that make genuine emergence possible (spatial
heterogeneity, trade-offs, isolation→speciation, trophic levels, disturbance).

## Status

| Plan | What | State |
|------|------|-------|
| 1 | World substrate — 2.5D space (`Surface`/`Underground` layers) + diffusing valaar field | ✅ done |
| 2 | Organisms + ecology — trait-vector genome, energy economy, implicit selection | ✅ done |
| 3 | Climate + the 6-arh seasons — heat/water fields, tolerance genes, seasonal stress | ✅ done |
| 4 | Static geography — terrain map (cell type + valaar conductivity + passability), sketch-driven generator, interactive map viewer | 🚧 in progress |
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
  viewer and map I/O (plan 4) are the only places that pull in dependencies
  (`minifb` for the window, `serde`/`serde_json` for map files), kept out of the
  hot path.
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

Plan 4 adds:

```sh
# Generate the static world from a sketch and save it (out/alchaea.json)
cargo run --bin mapgen [sketch.json]

# Open the interactive map viewer: drag to pan, scroll to zoom,
# Tab toggles Surface/Underground, Esc quits.
cargo run --bin mapview [map.json]
```

## Layout

```
src/
  space.rs        2.5D spatial model (Coord, Layer, Space trait, Grid2p5D)
  field.rs        scalar field (one f32 per cell)
  valaar.rs       valaar dynamics (inject / diffuse / layer-exchange / decay)
  world.rs        World = Space + valaar field + sources/access points
  rng.rs          deterministic PRNG
  genome.rs       trait-vector genome (8 traits)
  organism.rs     Organism trait + TraitOrganism (first clade)
  population.rs    organism store + occupancy index
  params.rs       EcoParams — every ecology rate as a dial
  ecology.rs      tick functions (absorb / move / predate / stress / metabolize / cull / reproduce)
  season.rs       the 6-arh calendar
  climate.rs      season → heat/water targets + field relaxation
  sim.rs          Sim — weaves world + climate + ecology into one tick
  terrain.rs      (plan 4) CellType + TerrainMap + JSON I/O
  sketch.rs       (plan 4) parse a Tiled sketch
  worldgen.rs     (plan 4) sketch → terrain map (deterministic, lore-constrained)
  viewer.rs       (plan 4) pan/zoom camera + framebuffer renderer
  bin/            life, sim, mapgen, mapview
docs/
  plans/          implementation plans (one per plan)
  tile-design-guide.md
```

## License

This project is released into the **public domain** under
[The Unlicense](LICENSE) — do anything you like with it, no attribution required.
