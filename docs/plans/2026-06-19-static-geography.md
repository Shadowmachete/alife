# Static Geography (sketch-driven worldgen + map visualiser) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the world a fixed *abiotic stage* — a per-cell **terrain map** (continents, oceans, valaar rivers, the central Rasconne core) authored from a hand-drawn sketch by a deterministic, lore-constrained generator, saved to JSON, and rendered to PNG with the project's first real tile renderer.

**Architecture:** Layered on plans 1–3. A new `CellType` enum and a `TerrainMap` (a `Field`-shaped grid of `CellType` over the existing `Space`) describe the stage; each `CellType` carries a *valaar conductivity* and *passability* that **plan 5** will feed into diffusion and movement (this plan only authors and visualises the data — the ecology stays terrain-blind). A `worldgen` module reads a **Tiled** sketch (JSON), upscales it, stamps a central Rasconne, traces seeded rivers outward, and fades the periphery into the Dusk. A `render` module composites 32 px tile PNGs (with a solid-colour fallback) into one image per layer. A `mapgen` binary wires sketch → generate → save JSON → render PNGs. Generation is seeded and reproducible (canonical seed = "the Alchaea").

**Tech Stack:** Rust (edition 2021). This plan **admits three external crates, scoped to map I/O and rendering only** (`serde`, `serde_json`, `image`); the simulation engine (plans 1–3) stays std-only.

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** `~/dev/ideas/world-design.md` §F (geography: 3–5 continents from Alchaea's break-up; oceans = barriers; zones centre→edge **Rasconne core → river corridors → the Dusk → Underground**) and the plan-3 doc's "Plan 4 — static geography" section. Tile vocabulary + style: `docs/tile-design-guide.md`.

**Builds on (plans 1–3, shipped — exact interfaces consumed):**

- `space::{Coord, Layer, Space, Grid2p5D}` — `Layer::{Surface, Underground}`, `Grid2p5D::new(w, h)`, `Space::{width, height, len, index, in_bounds, planar_neighbors}`.
- `field::Field` — the shape `TerrainMap` parallels (flat `Vec`, indexed via `Space::index`).
- `rng::Rng` — `new(seed: u64)`, `next_u64`, `next_unit() -> f32` (in `[0,1)`), `next_range(lo, hi)`. The generator's only randomness; keeps determinism without a new RNG.
- `world::{World, Params}`, `valaar::{diffuse_planar, …}`, `sim::Sim` — **not touched here**; plan 5 wires terrain into them.

## Global Constraints

- **Language:** Rust, edition 2021.
- **Dependencies (scoped relaxation):** this plan adds exactly three crates, used **only** in the new `terrain` (JSON I/O), `sketch` (Tiled parse), and `render` (PNG) modules:
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `image = { version = "0.25", default-features = false, features = ["png"] }`
  No other crates. The plan-1/2/3 engine modules (`space`, `field`, `world`, `valaar`, `genome`, `ecology`, `sim`, `season`, `climate`, …) **must remain std-only** — none of them may `use serde::` or `use image::`.
- **Determinism:** `worldgen::generate(sketch, w, h, seed)` is a pure function of its inputs. Same sketch + dims + seed ⇒ byte-identical `TerrainMap`. Randomness comes only from `Rng::new(seed ^ salt)`. `serde_json` output is field-order-stable. Identical inputs ⇒ identical JSON and identical PNG.
- **Spatial access stays behind `Space`:** `TerrainMap` is a flat `Vec<CellType>` indexed via `Space::index`; `worldgen` and `render` iterate `(x, y, layer)` and call `space.index(...)`. No module assumes the `Grid2p5D` memory layout directly.
- **Terrain is the stage, not the simulation:** this plan **does not** modify `valaar::diffuse_planar` or `ecology::move_organisms`. `CellType::conductivity()` / `passable()` exist and are tested, but nothing consumes them yet (plan 5 does). No `EcoParams`/`Sim` changes.
- **Runs before the art exists:** the renderer falls back to solid palette colours when a tile PNG is missing, so the whole pipeline is runnable and testable now. Real tiles drop into `assets/tiles/<stem>.png` later with zero code change.
- **Version control:** commit per task. Plain commit messages, **no `Co-Authored-By` trailer** (global user preference).

**Scope (plan 4 of several) — what's DEFERRED, and to which plan:**

- **Plan 5 — terrain-aware ecology (the payoff).** Make `valaar::diffuse_planar` weight exchange by `CellType::conductivity()` (rivers conduct, oceans/mountains block) and `ecology::move_organisms` respect `passable()` (oceans become real barriers → allopatric speciation). This is why conductivity/passability are authored now.
- **Plan 5+ — the dynamic world:** Vraze land-bridges + earthquakes mutating the terrain; underground reservoirs/caves as a climate refuge + the `digging` gene; the valaar state-machine.
- **Deferred tiles:** `coast` (land/ocean transition) and `cave` (underground access) `CellType`s — add when needed; the enum is `non_exhaustive`-friendly to extend.

---

## Design assumptions (flagged for veto)

- **A1 — CellType set:** `{ Ocean, Land, River, Rock, Mountain, Rasconne }` — the tile-guide minimum set plus `Rock` (barren/Dusk ground). `Coast`/`Cave` deferred.
- **A2 — conductivity/passability table** (Task 1): `Ocean 0.0/✗`, `Mountain 0.0/✗`, `Rock 0.3/✓`, `Land 1.0/✓`, `River 1.5/✓`, `Rasconne 2.0/✓`. Concrete and tunable; only *meaning* is fixed (oceans/mountains block valaar and movement; rivers/Rasconne conduct best). Consumed in plan 5.
- **A3 — central Rasconne:** the generator stamps a Rasconne disk (radius 2) at the *map centre* regardless of the sketch, enforcing the "continents split outward from a central Rasconne" lore.
- **A4 — rivers:** 6 seeded, gently-meandering walks from the core outward, stopping at ocean/mountain/edge. A structural + visual feature now; their high conductivity matters in plan 5.
- **A5 — the Dusk:** surface `Land` beyond `0.55 ×` the half-min-dimension radius becomes `Rock`, painting the centre→edge oligotrophic gradient.
- **A6 — underground layer:** filled uniformly with `Rock` (placeholder). Real underground geography (reservoirs/caves/access points) is plan 5+.
- **A7 — sketch contract:** the Tiled tileset is laid out so global tile IDs `1..=6` map to `CellType::ALL` order (`1=Ocean, 2=Land, 3=River, 4=Rock, 5=Mountain, 6=Rasconne`); GID `0`/unknown ⇒ `Ocean`. The generator reads the **first tile layer**. (You control the sketch; this is the agreed convention.)
- **A8 — map JSON schema:** `{ "version": 1, "w", "h", "layers": 2, "seed", "cells": ["ocean", …] }`; `cells` covers **both** layers in `Space::index` order (layer-major: surface block then underground block).
- **A9 — tile size:** 32 px (matches `docs/tile-design-guide.md`). Renderer nearest-neighbour-resizes any off-size tile and falls back to `CellType::fallback_rgb()` when a tile file is absent.
- **A10 — deps:** `serde`/`serde_json`/`image` admitted, scoped as in Global Constraints.

---

## File Structure

- `Cargo.toml` — **modified**: add the three dependencies (incrementally, in the task that first needs each).
- `.gitignore` — **modified**: ignore generated `/out`.
- `src/lib.rs` — **modified**: add `pub mod terrain; pub mod sketch; pub mod worldgen; pub mod render;`.
- `src/terrain.rs` — **new**: `CellType` (+ properties, serde) and `TerrainMap` (+ `save_json`/`load_json`).
- `src/sketch.rs` — **new**: parse a Tiled JSON export into a coarse `Sketch` of region seeds.
- `src/worldgen.rs` — **new**: `generate(sketch, w, h, seed) -> TerrainMap` and its lore steps.
- `src/render.rs` — **new**: composite tile PNGs (with fallback) into an `image::RgbImage` per layer.
- `src/bin/mapgen.rs` — **new**: CLI — sketch → generate (canonical seed) → save JSON → render PNGs.
- `assets/sketch.json` — **new** (Task 8): a small starter Tiled sketch so the pipeline runs before your real sketch arrives.

Each `src/lib.rs` edit adds one `pub mod`, shown in its task.

---

### Task 1: `CellType` — the terrain vocabulary

**Files:**
- Create: `src/terrain.rs` (the `CellType` enum + its tests)
- Modify: `src/lib.rs` (add `pub mod terrain;`)
- Modify: `Cargo.toml` (add `serde`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum CellType { Ocean, Land, River, Rock, Mountain, Rasconne }` deriving `Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize` (serde `rename_all = "lowercase"`).
  - `impl CellType`: `pub const ALL: [CellType; 6]`; `pub fn conductivity(self) -> f32`; `pub fn passable(self) -> bool`; `pub fn code(self) -> char`; `pub fn from_code(c: char) -> Option<CellType>`; `pub fn tile_stem(self) -> &'static str`; `pub fn fallback_rgb(self) -> [u8; 3]`.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]`:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
```

- [ ] **Step 2: Add the module**

Append to `src/lib.rs`:

```rust
pub mod terrain;
```

- [ ] **Step 3: Write the failing tests**

Create `src/terrain.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_round_trips_for_every_type() {
        for t in CellType::ALL {
            assert_eq!(CellType::from_code(t.code()), Some(t), "round-trip {t:?}");
        }
        assert_eq!(CellType::from_code('?'), None);
    }

    #[test]
    fn oceans_and_mountains_block_movement_and_valaar() {
        assert!(!CellType::Ocean.passable());
        assert!(!CellType::Mountain.passable());
        assert_eq!(CellType::Ocean.conductivity(), 0.0);
        assert_eq!(CellType::Mountain.conductivity(), 0.0);
    }

    #[test]
    fn rivers_and_the_core_conduct_better_than_land() {
        assert!(CellType::River.conductivity() > CellType::Land.conductivity());
        assert!(CellType::Rasconne.conductivity() >= CellType::River.conductivity());
        assert!(CellType::Land.passable());
        assert!(CellType::Rasconne.passable());
    }

    #[test]
    fn every_type_has_a_distinct_tile_stem() {
        let mut seen = std::collections::HashSet::new();
        for t in CellType::ALL {
            assert!(seen.insert(t.tile_stem()), "duplicate stem for {t:?}");
            assert!(!t.tile_stem().is_empty());
        }
    }
}
```

- [ ] **Step 4: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: **compile error** — `cannot find type CellType`.

- [ ] **Step 5: Write the minimal implementation**

Insert above the `tests` module in `src/terrain.rs`:

```rust
//! Static geography — the abiotic stage. `CellType` names what fills a cell and
//! carries the two properties plan 5's ecology will consume: valaar
//! `conductivity` (how freely valaar diffuses through it) and `passable` (whether
//! an organism may move into it). `TerrainMap` (below) is a grid of these.
//!
//! Authored by `worldgen`, persisted as JSON here, drawn by `render`.

use serde::{Deserialize, Serialize};

/// What fills a cell. Tile/file names are the lowercase variant (`tile_stem`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CellType {
    Ocean,
    Land,
    River,
    Rock,
    Mountain,
    Rasconne,
}

impl CellType {
    pub const ALL: [CellType; 6] = [
        CellType::Ocean,
        CellType::Land,
        CellType::River,
        CellType::Rock,
        CellType::Mountain,
        CellType::Rasconne,
    ];

    /// How freely valaar diffuses through this cell (0 = blocks). A multiplier on
    /// diffusion, consumed by plan 5; carried here so the map is self-describing.
    /// [ASSUMPTION A2]
    pub fn conductivity(self) -> f32 {
        match self {
            CellType::Ocean => 0.0,
            CellType::Mountain => 0.0,
            CellType::Rock => 0.3,
            CellType::Land => 1.0,
            CellType::River => 1.5,
            CellType::Rasconne => 2.0,
        }
    }

    /// Whether an organism may move into this cell (plan 5 consumes this).
    pub fn passable(self) -> bool {
        !matches!(self, CellType::Ocean | CellType::Mountain)
    }

    /// One-char code for compact text / debugging.
    pub fn code(self) -> char {
        match self {
            CellType::Ocean => 'O',
            CellType::Land => '.',
            CellType::River => '~',
            CellType::Rock => ':',
            CellType::Mountain => '^',
            CellType::Rasconne => 'R',
        }
    }

    pub fn from_code(c: char) -> Option<CellType> {
        match c {
            'O' => Some(CellType::Ocean),
            '.' => Some(CellType::Land),
            '~' => Some(CellType::River),
            ':' => Some(CellType::Rock),
            '^' => Some(CellType::Mountain),
            'R' => Some(CellType::Rasconne),
            _ => None,
        }
    }

    /// Tile filename stem → `assets/tiles/<stem>.png`.
    pub fn tile_stem(self) -> &'static str {
        match self {
            CellType::Ocean => "ocean",
            CellType::Land => "land",
            CellType::River => "river",
            CellType::Rock => "rock",
            CellType::Mountain => "mountain",
            CellType::Rasconne => "rasconne",
        }
    }

    /// Solid colour `[r, g, b]` used when a tile PNG is absent (palette per
    /// `docs/tile-design-guide.md`).
    pub fn fallback_rgb(self) -> [u8; 3] {
        match self {
            CellType::Ocean => [20, 28, 64],
            CellType::Land => [96, 120, 72],
            CellType::River => [40, 180, 170],
            CellType::Rock => [96, 92, 110],
            CellType::Mountain => [120, 120, 130],
            CellType::Rasconne => [220, 80, 60],
        }
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: `test result: ok. 4 passed`. (First build downloads `serde`.)

- [ ] **Step 7: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: CellType terrain vocabulary (conductivity, passability, tiles)"
```

---

### Task 2: `TerrainMap` — a grid of `CellType`

**Files:**
- Modify: `src/terrain.rs` (add `TerrainMap` + tests)

**Interfaces:**
- Consumes: `CellType` (Task 1); `Space` (plan 1).
- Produces:
  - `pub struct TerrainMap` deriving `Clone, Debug, PartialEq, Serialize, Deserialize`, with serde field names `w`, `h`, `layers`, `seed`, `cells`.
  - `pub fn filled(len: usize, width: u32, height: u32, fill: CellType, seed: u64) -> TerrainMap`.
  - `pub fn width(&self) -> u32`, `height(&self) -> u32`, `layers(&self) -> u32`, `seed(&self) -> u64`, `len(&self) -> usize`, `is_empty(&self) -> bool`.
  - `pub fn get(&self, i: usize) -> CellType`, `pub fn set(&mut self, i: usize, t: CellType)`, `pub fn cells(&self) -> &[CellType]`.

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/terrain.rs` (after the existing tests):

```rust
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    #[test]
    fn filled_map_is_uniform_and_sized_to_space() {
        let space = Grid2p5D::new(4, 3);
        let map = TerrainMap::filled(space.len(), 4, 3, CellType::Ocean, 42);
        assert_eq!(map.len(), space.len());
        assert_eq!(map.width(), 4);
        assert_eq!(map.height(), 3);
        assert_eq!(map.seed(), 42);
        for i in 0..map.len() {
            assert_eq!(map.get(i), CellType::Ocean);
        }
    }

    #[test]
    fn set_get_round_trips_through_space_index() {
        let space = Grid2p5D::new(4, 3);
        let mut map = TerrainMap::filled(space.len(), 4, 3, CellType::Ocean, 0);
        let c = Coord::new(2, 1, Layer::Surface);
        map.set(space.index(c), CellType::Rasconne);
        assert_eq!(map.get(space.index(c)), CellType::Rasconne);
        // a different cell is untouched
        assert_eq!(map.get(space.index(Coord::new(0, 0, Layer::Underground))), CellType::Ocean);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: **compile error** — `cannot find type TerrainMap`.

- [ ] **Step 3: Write the minimal implementation**

Add to `src/terrain.rs`, below the `CellType` impl and above the `tests` module:

```rust
/// A `CellType` per cell, sized to a `Space` (all layers), plus the metadata
/// needed to persist and render it standalone. Parallels `field::Field`: a flat
/// vector indexed via `Space::index`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainMap {
    #[serde(rename = "w")]
    width: u32,
    #[serde(rename = "h")]
    height: u32,
    layers: u32,
    seed: u64,
    cells: Vec<CellType>,
}

impl TerrainMap {
    /// A map of `len` cells (= `space.len()`) all set to `fill`.
    pub fn filled(len: usize, width: u32, height: u32, fill: CellType, seed: u64) -> Self {
        let plane = width as usize * height as usize;
        let layers = if plane == 0 { 0 } else { (len / plane) as u32 };
        TerrainMap { width, height, layers, seed, cells: vec![fill; len] }
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn layers(&self) -> u32 {
        self.layers
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn get(&self, i: usize) -> CellType {
        self.cells[i]
    }
    pub fn set(&mut self, i: usize, t: CellType) {
        self.cells[i] = t;
    }
    pub fn cells(&self) -> &[CellType] {
        &self.cells
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: `test result: ok. 6 passed`.

- [ ] **Step 5: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: TerrainMap — a Space-indexed grid of CellType"
```

---

### Task 3: Map persistence (JSON save/load)

**Files:**
- Modify: `src/terrain.rs` (add `save_json` / `load_json` + tests)
- Modify: `Cargo.toml` (add `serde_json`)

**Interfaces:**
- Consumes: `TerrainMap` (Task 2).
- Produces:
  - `pub fn save_json(map: &TerrainMap, path: &std::path::Path) -> std::io::Result<()>`.
  - `pub fn load_json(path: &std::path::Path) -> std::io::Result<TerrainMap>`.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]` (alongside `serde`):

```toml
serde_json = "1"
```

- [ ] **Step 2: Write the failing tests**

Add inside the `tests` module in `src/terrain.rs`:

```rust
    #[test]
    fn json_round_trips_through_a_file() {
        let space = Grid2p5D::new(5, 4);
        let mut map = TerrainMap::filled(space.len(), 5, 4, CellType::Land, 0xABCD);
        map.set(space.index(Coord::new(2, 2, Layer::Surface)), CellType::Rasconne);
        map.set(space.index(Coord::new(0, 0, Layer::Surface)), CellType::Ocean);

        let path = std::env::temp_dir().join("alife_terrain_roundtrip.json");
        save_json(&map, &path).unwrap();
        let loaded = load_json(&path).unwrap();
        assert_eq!(loaded, map);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_uses_lowercase_type_names() {
        let space = Grid2p5D::new(1, 1);
        let map = TerrainMap::filled(space.len(), 1, 1, CellType::Ocean, 0);
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains("\"ocean\""), "got {json}");
        assert!(json.contains("\"w\":1"));
    }
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: **compile error** — `cannot find function save_json` / unresolved `serde_json`.

- [ ] **Step 4: Write the minimal implementation**

Add to `src/terrain.rs`, below the `TerrainMap` impl and above the `tests` module:

```rust
use std::io;
use std::path::Path;

/// Write `map` as JSON (`{version, w, h, layers, seed, cells:[...]}`).
pub fn save_json(map: &TerrainMap, path: &Path) -> io::Result<()> {
    let json = serde_json::to_string(map)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

/// Read a `TerrainMap` back from a JSON file written by `save_json`.
pub fn load_json(path: &Path) -> io::Result<TerrainMap> {
    let s = std::fs::read_to_string(path)?;
    serde_json::from_str(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: `test result: ok. 8 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: TerrainMap JSON save/load"
```

---

### Task 4: Parse the Tiled sketch

**Files:**
- Create: `src/sketch.rs`
- Modify: `src/lib.rs` (add `pub mod sketch;`)

**Interfaces:**
- Consumes: `CellType` (Task 1); `serde_json` (Task 3).
- Produces:
  - `pub struct Sketch { pub width: u32, pub height: u32, pub regions: Vec<CellType> }`.
  - `pub fn parse_sketch(json: &str) -> Result<Sketch, String>` — reads the first tile layer of a Tiled JSON export, mapping GID `1..=6` to `CellType::ALL` (GID `0`/unknown ⇒ `Ocean`), masking Tiled's flip-flag high bits. `[ASSUMPTION A7]`

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod sketch;
```

- [ ] **Step 2: Write the failing tests**

Create `src/sketch.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::CellType;

    const FIXTURE: &str = r#"{
        "width": 3, "height": 3,
        "layers": [
            { "type": "tilelayer", "name": "sketch", "width": 3, "height": 3,
              "data": [1, 1, 1, 1, 6, 1, 1, 2, 5] }
        ]
    }"#;

    #[test]
    fn parses_dimensions_and_maps_gids_to_celltypes() {
        let s = parse_sketch(FIXTURE).unwrap();
        assert_eq!(s.width, 3);
        assert_eq!(s.height, 3);
        assert_eq!(s.regions.len(), 9);
        assert_eq!(s.regions[0], CellType::Ocean); // gid 1
        assert_eq!(s.regions[4], CellType::Rasconne); // gid 6 (centre)
        assert_eq!(s.regions[7], CellType::Land); // gid 2
        assert_eq!(s.regions[8], CellType::Mountain); // gid 5
    }

    #[test]
    fn flip_flag_high_bits_are_ignored() {
        // Tiled sets the top 3 bits for flipped tiles; gid 6 flipped is still Rasconne.
        let flipped = 6u32 | 0x8000_0000;
        let json = format!(
            r#"{{"width":1,"height":1,"layers":[{{"type":"tilelayer","width":1,"height":1,"data":[{flipped}]}}]}}"#
        );
        let s = parse_sketch(&json).unwrap();
        assert_eq!(s.regions[0], CellType::Rasconne);
    }

    #[test]
    fn errors_when_there_is_no_tile_layer() {
        let json = r#"{"width":2,"height":2,"layers":[]}"#;
        assert!(parse_sketch(json).is_err());
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib sketch`
Expected: **compile error** — `cannot find function parse_sketch`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/sketch.rs`:

```rust
//! Read a hand-drawn map sketch (a Tiled JSON export) into a coarse grid of
//! `CellType` "region seeds" that `worldgen` upscales and fleshes out.
//!
//! Contract [A7]: the sketch's tileset places tiles so global IDs 1..=6 map to
//! `CellType::ALL` order; GID 0/unknown ⇒ Ocean. We read the first tile layer.

use crate::terrain::CellType;
use serde::Deserialize;

/// A coarse map of region seeds, row-major (`width × height`).
pub struct Sketch {
    pub width: u32,
    pub height: u32,
    pub regions: Vec<CellType>,
}

#[derive(Deserialize)]
struct TiledMap {
    layers: Vec<TiledLayer>,
}

#[derive(Deserialize)]
struct TiledLayer {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    data: Vec<u32>,
}

/// Map a Tiled global tile ID to a region seed (flip-flag bits masked off).
fn region_from_gid(gid: u32) -> CellType {
    match gid & 0x1FFF_FFFF {
        1 => CellType::Ocean,
        2 => CellType::Land,
        3 => CellType::River,
        4 => CellType::Rock,
        5 => CellType::Mountain,
        6 => CellType::Rasconne,
        _ => CellType::Ocean,
    }
}

pub fn parse_sketch(json: &str) -> Result<Sketch, String> {
    let map: TiledMap = serde_json::from_str(json).map_err(|e| format!("bad Tiled JSON: {e}"))?;
    let layer = map
        .layers
        .iter()
        .find(|l| l.kind == "tilelayer" && !l.data.is_empty())
        .ok_or_else(|| "no non-empty tile layer found".to_string())?;
    if (layer.width as usize * layer.height as usize) != layer.data.len() {
        return Err(format!(
            "layer size {}x{} does not match {} tiles",
            layer.width,
            layer.height,
            layer.data.len()
        ));
    }
    let regions = layer.data.iter().map(|&gid| region_from_gid(gid)).collect();
    Ok(Sketch { width: layer.width, height: layer.height, regions })
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib sketch`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: parse Tiled JSON sketch into coarse region seeds"
```

---

### Task 5: Generator core (upscale + central Rasconne + underground)

**Files:**
- Create: `src/worldgen.rs`
- Modify: `src/lib.rs` (add `pub mod worldgen;`)

**Interfaces:**
- Consumes: `Sketch` (Task 4); `TerrainMap`/`CellType` (Tasks 1–2); `space::{Coord, Grid2p5D, Layer, Space}` (plan 1).
- Produces:
  - `pub const ALCHAEA_SEED: u64 = 0x00A1_C4EA;`
  - `pub fn generate(sketch: &Sketch, width: u32, height: u32, seed: u64) -> TerrainMap`.
  - (private helpers `upscale_surface`, `stamp_rasconne`, `fill_underground` — rivers/Dusk arrive in Task 6.)

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod worldgen;
```

- [ ] **Step 2: Write the failing tests**

Create `src/worldgen.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::Sketch;
    use crate::space::{Coord, Grid2p5D, Layer, Space};
    use crate::terrain::CellType;

    // A coarse sketch: ocean border, land interior.
    fn land_in_ocean(cw: u32, ch: u32) -> Sketch {
        let mut regions = vec![CellType::Land; (cw * ch) as usize];
        for x in 0..cw {
            regions[x as usize] = CellType::Ocean; // top row
            regions[((ch - 1) * cw + x) as usize] = CellType::Ocean; // bottom row
        }
        for y in 0..ch {
            regions[(y * cw) as usize] = CellType::Ocean; // left col
            regions[(y * cw + cw - 1) as usize] = CellType::Ocean; // right col
        }
        Sketch { width: cw, height: ch, regions }
    }

    #[test]
    fn map_is_sized_to_the_space_with_both_layers() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(map.len(), space.len());
        assert_eq!(map.width(), 24);
        assert_eq!(map.height(), 24);
        assert_eq!(map.layers(), 2);
    }

    #[test]
    fn rasconne_sits_at_the_centre() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(
            map.get(space.index(Coord::new(12, 12, Layer::Surface))),
            CellType::Rasconne
        );
    }

    #[test]
    fn ocean_border_survives_upscaling() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(
            map.get(space.index(Coord::new(0, 0, Layer::Surface))),
            CellType::Ocean
        );
    }

    #[test]
    fn underground_is_a_uniform_placeholder() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        for x in 0..24 {
            for y in 0..24 {
                assert_eq!(
                    map.get(space.index(Coord::new(x, y, Layer::Underground))),
                    CellType::Rock
                );
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        let b = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(a, b, "same sketch + seed must give an identical map");
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib worldgen`
Expected: **compile error** — `cannot find function generate` / `ALCHAEA_SEED`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/worldgen.rs`:

```rust
//! The lore-constrained, deterministic world generator. It takes a coarse
//! `Sketch` and fleshes it into a full-resolution `TerrainMap`: upscale the
//! sketch, stamp a central Rasconne core, (Task 6) trace valaar rivers outward
//! and fade the periphery into the Dusk, and fill the underground placeholder.
//!
//! Pure function of `(sketch, width, height, seed)` — the only randomness is an
//! `Rng` seeded from `seed`, so the canonical `ALCHAEA_SEED` always rebuilds the
//! same world.

use crate::sketch::Sketch;
use crate::space::{Coord, Grid2p5D, Layer, Space};
use crate::terrain::{CellType, TerrainMap};

/// The canonical seed — "the" Alchaea.
pub const ALCHAEA_SEED: u64 = 0x00A1_C4EA;

/// Radius (cells) of the Rasconne core stamped at the map centre. [A3]
const RASCONNE_RADIUS: i32 = 2;

/// Generate a full-resolution terrain map from a coarse sketch.
pub fn generate(sketch: &Sketch, width: u32, height: u32, seed: u64) -> TerrainMap {
    let space = Grid2p5D::new(width, height);
    let mut map = TerrainMap::filled(space.len(), width, height, CellType::Ocean, seed);
    upscale_surface(&space, &mut map, sketch);
    let center = (width / 2, height / 2);
    stamp_rasconne(&space, &mut map, center);
    fill_underground(&space, &mut map);
    map
}

/// Blow the coarse sketch up to full resolution by nearest-coarse-cell.
fn upscale_surface<S: Space>(space: &S, map: &mut TerrainMap, sketch: &Sketch) {
    let (w, h) = (map.width(), map.height());
    let (cw, ch) = (sketch.width.max(1), sketch.height.max(1));
    for y in 0..h {
        for x in 0..w {
            let cx = (x * cw / w).min(cw - 1);
            let cy = (y * ch / h).min(ch - 1);
            let region = sketch.regions[(cy * cw + cx) as usize];
            map.set(space.index(Coord::new(x, y, Layer::Surface)), region);
        }
    }
}

/// Stamp a Rasconne disk at the map centre, enforcing the central-core lore. [A3]
fn stamp_rasconne<S: Space>(space: &S, map: &mut TerrainMap, center: (u32, u32)) {
    let (cx, cy) = (center.0 as i32, center.1 as i32);
    let (w, h) = (map.width() as i32, map.height() as i32);
    for dy in -RASCONNE_RADIUS..=RASCONNE_RADIUS {
        for dx in -RASCONNE_RADIUS..=RASCONNE_RADIUS {
            if dx * dx + dy * dy > RASCONNE_RADIUS * RASCONNE_RADIUS {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            map.set(
                space.index(Coord::new(x as u32, y as u32, Layer::Surface)),
                CellType::Rasconne,
            );
        }
    }
}

/// Fill the underground layer with a uniform placeholder. [A6]
fn fill_underground<S: Space>(space: &S, map: &mut TerrainMap) {
    let (w, h) = (map.width(), map.height());
    for y in 0..h {
        for x in 0..w {
            map.set(
                space.index(Coord::new(x, y, Layer::Underground)),
                CellType::Rock,
            );
        }
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib worldgen`
Expected: `test result: ok. 5 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: worldgen core — upscale sketch + central Rasconne + underground"
```

---

### Task 6: Rivers + the Dusk

**Files:**
- Modify: `src/worldgen.rs` (add `trace_rivers` + `mark_dusk`, call them in `generate`, add tests)

**Interfaces:**
- Consumes: everything in Task 5 + `rng::Rng` (plan 1).
- Produces: no new public API — `generate` now also traces rivers and fades the Dusk. Private `trace_rivers`, `mark_dusk`.

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/worldgen.rs`:

```rust
    fn surface_counts(map: &TerrainMap, space: &Grid2p5D) -> std::collections::HashMap<CellType, u32> {
        let mut counts = std::collections::HashMap::new();
        for y in 0..map.height() {
            for x in 0..map.width() {
                let t = map.get(space.index(Coord::new(x, y, Layer::Surface)));
                *counts.entry(t).or_insert(0) += 1;
            }
        }
        counts
    }

    fn all_land(cw: u32, ch: u32) -> Sketch {
        Sketch { width: cw, height: ch, regions: vec![CellType::Land; (cw * ch) as usize] }
    }

    #[test]
    fn rivers_and_dusk_appear_on_an_all_land_continent() {
        let space = Grid2p5D::new(40, 40);
        let map = generate(&all_land(4, 4), 40, 40, ALCHAEA_SEED);
        let counts = surface_counts(&map, &space);
        assert!(counts.get(&CellType::River).copied().unwrap_or(0) > 0, "rivers should be traced");
        assert!(counts.get(&CellType::Rock).copied().unwrap_or(0) > 0, "the Dusk should appear at the edge");
        assert!(counts.get(&CellType::Land).copied().unwrap_or(0) > 0, "the core ring stays land");
    }

    #[test]
    fn rivers_never_overwrite_the_ocean() {
        // ocean border preserved: rivers stop at the sea, never paint it.
        let space = Grid2p5D::new(40, 40);
        let mut bordered = vec![CellType::Land; 16];
        for k in 0..4 {
            bordered[k] = CellType::Ocean; // top
            bordered[12 + k] = CellType::Ocean; // bottom
            bordered[k * 4] = CellType::Ocean; // left
            bordered[k * 4 + 3] = CellType::Ocean; // right
        }
        let sketch = Sketch { width: 4, height: 4, regions: bordered };
        let map = generate(&sketch, 40, 40, ALCHAEA_SEED);
        assert_eq!(map.get(space.index(Coord::new(0, 0, Layer::Surface))), CellType::Ocean);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib worldgen`
Expected: **FAIL** — `rivers_and_dusk_appear_on_an_all_land_continent` finds no `River`/`Rock` (generate doesn't trace them yet). `rivers_never_overwrite_the_ocean` may already pass; that is fine.

- [ ] **Step 3: Write the implementation**

In `src/worldgen.rs`, add `use crate::rng::Rng;` to the imports, add these consts near `RASCONNE_RADIUS`:

```rust
/// Number of valaar rivers traced from the core. [A4]
const RIVER_COUNT: u32 = 6;
/// Maximum length (steps) of each river. [A4]
const RIVER_MAX_LEN: u32 = 600;
/// Decorrelates the river RNG from any other use of the seed.
const RIVER_SALT: u64 = 0x5249_5645_52; // "RIVER"
/// Surface land beyond this fraction of the half-min-dimension becomes Dusk rock. [A5]
const DUSK_FRACTION: f32 = 0.55;
```

Insert the two calls into `generate`, between `stamp_rasconne` and `fill_underground`:

```rust
    stamp_rasconne(&space, &mut map, center);
    trace_rivers(&space, &mut map, center, seed);
    mark_dusk(&space, &mut map, center);
    fill_underground(&space, &mut map);
```

Add the two functions (above the `tests` module):

```rust
/// Trace seeded, gently-meandering rivers from the core outward. Each stops at
/// ocean, a mountain, or the edge; rivers flow through (not over) Rasconne. [A4]
fn trace_rivers<S: Space>(space: &S, map: &mut TerrainMap, center: (u32, u32), seed: u64) {
    let mut rng = Rng::new(seed ^ RIVER_SALT);
    let (w, h) = (map.width() as i32, map.height() as i32);
    for _ in 0..RIVER_COUNT {
        let mut angle = rng.next_unit() * std::f32::consts::TAU;
        let mut fx = center.0 as f32;
        let mut fy = center.1 as f32;
        for _ in 0..RIVER_MAX_LEN {
            angle += (rng.next_unit() - 0.5) * 0.5; // gentle meander
            fx += angle.cos();
            fy += angle.sin();
            let (x, y) = (fx.round() as i32, fy.round() as i32);
            if x < 0 || y < 0 || x >= w || y >= h {
                break;
            }
            let i = space.index(Coord::new(x as u32, y as u32, Layer::Surface));
            match map.get(i) {
                CellType::Ocean | CellType::Mountain => break,
                CellType::Rasconne => continue,
                _ => map.set(i, CellType::River),
            }
        }
    }
}

/// Fade surface `Land` beyond the Dusk radius into `Rock` — the oligotrophic
/// periphery. Rivers, ocean, mountains, and the core are left alone. [A5]
fn mark_dusk<S: Space>(space: &S, map: &mut TerrainMap, center: (u32, u32)) {
    let (w, h) = (map.width(), map.height());
    let threshold = DUSK_FRACTION * (w.min(h) as f32) * 0.5;
    for y in 0..h {
        for x in 0..w {
            let i = space.index(Coord::new(x, y, Layer::Surface));
            if map.get(i) != CellType::Land {
                continue;
            }
            let dx = x as f32 - center.0 as f32;
            let dy = y as f32 - center.1 as f32;
            if (dx * dx + dy * dy).sqrt() > threshold {
                map.set(i, CellType::Rock);
            }
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib worldgen`
Expected: `test result: ok. 7 passed` (the 5 from Task 5 stay green — rivers stop at ocean, so the all-ocean-border and underground tests are unaffected).

- [ ] **Step 5: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: worldgen traces valaar rivers + fades the Dusk"
```

---

### Task 7: PNG tile renderer

**Files:**
- Create: `src/render.rs`
- Modify: `src/lib.rs` (add `pub mod render;`)
- Modify: `Cargo.toml` (add `image`)

**Interfaces:**
- Consumes: `TerrainMap`/`CellType` (Tasks 1–2); `space::{Coord, Layer, Space}` (plan 1).
- Produces:
  - `pub const TILE: u32 = 32;`
  - `pub fn render_layer<S: Space>(map: &TerrainMap, space: &S, layer: Layer, tiles_dir: &std::path::Path) -> image::RgbImage`.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]`:

```toml
image = { version = "0.25", default-features = false, features = ["png"] }
```

- [ ] **Step 2: Add the module**

Append to `src/lib.rs`:

```rust
pub mod render;
```

- [ ] **Step 3: Write the failing tests**

Create `src/render.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Coord, Grid2p5D, Layer, Space};
    use crate::terrain::{CellType, TerrainMap};

    #[test]
    fn output_is_tile_sized_and_uses_the_fallback_colour() {
        let space = Grid2p5D::new(2, 2);
        let mut map = TerrainMap::filled(space.len(), 2, 2, CellType::Ocean, 0);
        map.set(space.index(Coord::new(1, 0, Layer::Surface)), CellType::Rasconne);
        // a tiles dir that does not exist => every tile uses fallback_rgb
        let tiles_dir = std::env::temp_dir().join("alife_no_such_tiles_dir_q1w2e3");
        let img = render_layer(&map, &space, Layer::Surface, &tiles_dir);

        assert_eq!(img.width(), 2 * TILE);
        assert_eq!(img.height(), 2 * TILE);
        // centre of cell (0,0) = Ocean fallback
        assert_eq!(img.get_pixel(TILE / 2, TILE / 2).0, CellType::Ocean.fallback_rgb());
        // centre of cell (1,0) = Rasconne fallback
        assert_eq!(
            img.get_pixel(TILE + TILE / 2, TILE / 2).0,
            CellType::Rasconne.fallback_rgb()
        );
    }
}
```

- [ ] **Step 4: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib render`
Expected: **compile error** — `cannot find function render_layer` / unresolved `image`.

- [ ] **Step 5: Write the minimal implementation**

Insert above the `tests` module in `src/render.rs`:

```rust
//! The first real renderer: composite per-`CellType` tiles into one PNG-ready
//! image per layer. Tiles are loaded from `assets/tiles/<stem>.png`; any missing
//! (or off-size) tile falls back to `CellType::fallback_rgb`, so the map renders
//! before the art exists. CPU-only; the live/GPU renderer is a later plan.

use crate::space::{Coord, Layer, Space};
use crate::terrain::{CellType, TerrainMap};
use image::{Rgb, RgbImage};
use std::collections::HashMap;
use std::path::Path;

/// Edge length of one tile, in pixels. [A9]
pub const TILE: u32 = 32;

/// Load the tile for `t` (resizing to `TILE` if needed), or synthesize a solid
/// fallback when the PNG is absent/unreadable.
fn tile_image(t: CellType, tiles_dir: &Path) -> RgbImage {
    let path = tiles_dir.join(format!("{}.png", t.tile_stem()));
    if let Ok(img) = image::open(&path) {
        let rgb = img.to_rgb8();
        if rgb.width() == TILE && rgb.height() == TILE {
            return rgb;
        }
        return image::imageops::resize(&rgb, TILE, TILE, image::imageops::FilterType::Nearest);
    }
    let [r, g, b] = t.fallback_rgb();
    RgbImage::from_pixel(TILE, TILE, Rgb([r, g, b]))
}

/// Composite one layer of `map` into an `RgbImage` (`width·TILE × height·TILE`).
/// Save it with `img.save("path.png")`.
pub fn render_layer<S: Space>(
    map: &TerrainMap,
    space: &S,
    layer: Layer,
    tiles_dir: &Path,
) -> RgbImage {
    let (w, h) = (map.width(), map.height());
    let mut canvas = RgbImage::new(w * TILE, h * TILE);
    let mut cache: HashMap<CellType, RgbImage> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let t = map.get(space.index(Coord::new(x, y, layer)));
            let tile = cache.entry(t).or_insert_with(|| tile_image(t, tiles_dir));
            image::imageops::overlay(&mut canvas, tile, (x * TILE) as i64, (y * TILE) as i64);
        }
    }
    canvas
}
```

- [ ] **Step 6: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib render`
Expected: `test result: ok. 1 passed`. (First build downloads `image`.)

- [ ] **Step 7: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: PNG tile renderer (tiles + solid-colour fallback)"
```

---

### Task 8: The `mapgen` binary + a starter sketch

**Files:**
- Create: `src/bin/mapgen.rs`
- Create: `assets/sketch.json`
- Modify: `.gitignore` (ignore `/out`)

**Interfaces:**
- Consumes: `sketch::parse_sketch`, `worldgen::{generate, ALCHAEA_SEED}`, `terrain::save_json`, `render::render_layer`, `space::{Grid2p5D, Layer}`.
- Produces: no library API — the end-to-end demo and verification.

- [ ] **Step 1: Ignore generated output**

Append to `.gitignore`:

```
/out
```

- [ ] **Step 2: Add a starter sketch**

Create `assets/sketch.json` (a 12×6 Tiled-style sketch: ocean border, land interior, two mountains; GID convention `1=Ocean, 2=Land, 5=Mountain` per [A7]). The generator adds Rasconne, rivers, and the Dusk:

```json
{
  "width": 12,
  "height": 6,
  "tilewidth": 32,
  "tileheight": 32,
  "layers": [
    {
      "type": "tilelayer",
      "name": "sketch",
      "width": 12,
      "height": 6,
      "data": [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1,
        1, 2, 2, 2, 2, 2, 2, 2, 5, 2, 2, 1,
        1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1,
        1, 2, 2, 2, 5, 2, 2, 2, 2, 2, 2, 1,
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1
      ]
    }
  ]
}
```

- [ ] **Step 3: Write the binary**

Create `src/bin/mapgen.rs`:

```rust
//! Generate the static world from a sketch and visualise it.
//! Usage: `cargo run --bin mapgen [sketch.json]` (default: assets/sketch.json).
//! Writes out/alchaea.json + out/alchaea_{surface,underground}.png.

use alife::render::render_layer;
use alife::space::{Grid2p5D, Layer};
use alife::terrain::save_json;
use alife::worldgen::{generate, ALCHAEA_SEED};
use std::path::Path;

fn main() {
    let sketch_path = std::env::args().nth(1).unwrap_or_else(|| "assets/sketch.json".into());
    let (w, h) = (120u32, 60u32);

    let json = std::fs::read_to_string(&sketch_path)
        .unwrap_or_else(|e| panic!("read sketch {sketch_path}: {e}"));
    let sketch = alife::sketch::parse_sketch(&json).expect("parse sketch");
    let map = generate(&sketch, w, h, ALCHAEA_SEED);

    std::fs::create_dir_all("out").expect("make out/");
    save_json(&map, Path::new("out/alchaea.json")).expect("save json");

    let space = Grid2p5D::new(w, h);
    let tiles = Path::new("assets/tiles");
    render_layer(&map, &space, Layer::Surface, tiles)
        .save("out/alchaea_surface.png")
        .expect("save surface png");
    render_layer(&map, &space, Layer::Underground, tiles)
        .save("out/alchaea_underground.png")
        .expect("save underground png");

    println!(
        "generated {w}x{h} map (seed {:#x}) -> out/alchaea.json + out/alchaea_*.png",
        map.seed()
    );
}
```

- [ ] **Step 4: Build, run, and verify the pipeline**

Run: `cd ~/dev/alife && cargo run --bin mapgen`
Expected: prints `generated 120x60 map (seed 0xa1c4ea) -> out/alchaea.json + out/alchaea_*.png`.

Run: `ls -la ~/dev/alife/out`
Expected: `alchaea.json`, `alchaea_surface.png` (3840×1920), `alchaea_underground.png` exist.

Run: `cd ~/dev/alife && cargo run --bin mapgen && cargo run --bin mapgen && cmp out/alchaea.json out/alchaea.json && echo DETERMINISTIC`
Expected: identical JSON across runs (determinism). (Inspect `out/alchaea_surface.png` by eye: a central red Rasconne, teal rivers radiating out, a green core fading to grey Rock at the edges, ocean border — solid-colour fallback until your tiles land in `assets/tiles/`.)

- [ ] **Step 5: Full suite + clippy**

Run: `cd ~/dev/alife && cargo test`
Expected: all green — plans 1–3 tests plus terrain (8), sketch (3), worldgen (7), render (1).

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: mapgen binary — sketch to JSON + PNG, with a starter sketch"
```

---

## What this delivers

A fixed abiotic stage for the world: a `CellType` terrain map carrying valaar conductivity + passability, generated deterministically from a hand-drawn **Tiled** sketch by a lore-constrained pipeline (central Rasconne, continents in ocean, valaar rivers radiating outward, a Dusk periphery), persisted as JSON, and visualised with the project's first real tile renderer (PNG per layer, real tiles when present, solid-colour fallback until then). The canonical `ALCHAEA_SEED` rebuilds the same world every time.

**How to use it with your assets:** drop your 32 px tiles into `assets/tiles/{ocean,land,river,rock,mountain,rasconne}.png` and re-run `cargo run --bin mapgen` — no code change. Hand me your real Tiled sketch and run `cargo run --bin mapgen path/to/your_sketch.json`.

**Next plan — Plan 5: terrain-aware ecology.** Feed `CellType::conductivity()` into `valaar::diffuse_planar` (rivers conduct, oceans/mountains block) and `CellType::passable()` into `ecology::move_organisms` (oceans become real barriers → allopatric speciation). Then the dynamic world: Vraze land-bridges + earthquakes mutating the terrain, underground reservoirs/caves + the `digging` gene, and the valaar state-machine.

## Self-review notes

- **Spec coverage:** terrain data model with conductivity + passability (Tasks 1–2, A1–A2); JSON persistence (Task 3, A8); Tiled sketch ingestion (Task 4, A7); lore-constrained deterministic generation — central Rasconne (Task 5, A3), upscaled continents/oceans (Task 5), valaar rivers (Task 6, A4), the Dusk gradient (Task 6, A5), underground placeholder (Task 5, A6); the first real renderer with tile + fallback paths (Task 7, A9); end-to-end binary + canonical seed (Task 8). Terrain→ecology integration explicitly deferred to plan 5 (per the decision that terrain-ecology is plan 5). Dynamic geography / underground / valaar-state deferred to plan 5+.
- **Type consistency:** `CellType` variants and their `conductivity/passable/code/from_code/tile_stem/fallback_rgb` are defined once (Task 1) and used unchanged in `TerrainMap`, `sketch`, `worldgen`, and `render`. `TerrainMap` accessors (`width/height/layers/seed/len/get/set/cells`) defined in Task 2 are the only surface used later. `generate(&Sketch, u32, u32, u64) -> TerrainMap`, `parse_sketch(&str) -> Result<Sketch, String>`, `render_layer(&TerrainMap, &S, Layer, &Path) -> RgbImage`, and `save_json/load_json(&Path)` signatures match across their definitions and call sites (incl. `bin/mapgen.rs`). The GID→CellType convention (1..=6 = `CellType::ALL`) is stated in A7, in `sketch::region_from_gid`, and in the `assets/sketch.json` fixture.
- **Determinism:** the only randomness is `Rng::new(seed ^ RIVER_SALT)` in `trace_rivers`; everything else is a pure function of the inputs. `serde_json` field order is stable. Task 5 and Task 8 assert byte-identical output across runs.
- **Dependency discipline:** `serde`/`serde_json`/`image` are confined to `terrain`, `sketch`, and `render`; the engine modules stay std-only (Global Constraints). `image` built with `default-features = false, features = ["png"]` to stay lean.
- **Placeholder scan:** every code step contains complete, compiling code; no TODO/TBD; all referenced types and functions are defined in this plan or in plans 1–3.
```