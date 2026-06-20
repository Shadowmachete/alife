# Terrain-Aware Movement, Continent Stats & egui Viewer Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix wrong behaviour and modernise the plan-4 viewer: organisms must stop walking onto Ocean/Valaar terrain; the viewer must show population statistics broken down by diet and by **continent**; and the window is rewritten on **egui/eframe** (WASM-ready) so organisms are legible (filled circles sized by body, coloured by diet, with an outline) and stats live in a side panel with Pause/Reseed controls.

**Architecture:** A thin slice of plan 5 ("terrain-aware ecology") plus a viewer rewrite. `CellType::passable()` gains `Valaar`; a per-cell passability mask lives on `World` and is consulted by `ecology::move_organisms`. Statistics are pure functions in `mapsim.rs` (`label_continents` flood-fills landmasses once at load — geography is static — and `compute_stats` tallies each refresh). The window shell moves from `minifb` to **`eframe`**: terrain is CPU-rendered into a pixel buffer (reusing the existing `viewer` renderers), uploaded as an egui texture each frame and drawn with a pan/zoom transform; organisms are drawn on top as `Painter` circles; a `SidePanel` shows stats + controls. The simulation engine and `src/viewer.rs` stay std-only and egui-free — egui lives only in the binary.

**Tech Stack:** Rust (edition 2021). Dependency change: **remove `minifb`, add `eframe`** (brings `egui`). `serde`/`serde_json` unchanged. The viewer will later be built for **WASM** (eframe's flagship target) to embed on the user's website; write the app as an `eframe::App` so the web build is a small follow-up.

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** `docs/plans/2026-06-19-static-geography.md` (plan 4, shipped) and its plan-5 deferral: *"gate `move_organisms` on `passable()` (oceans become real barriers → speciation)."* This plan does the movement gate; conductivity-weighted diffusion stays deferred.

**Builds on (shipped — exact interfaces consumed):**

- `space::{Coord, Layer, Space, Grid2p5D}` — `Coord{x,y,layer}`, `Space::{width, height, len, index, in_bounds, planar_neighbors}`.
- `terrain::{CellType, TerrainMap}` — `CellType` `{ Ocean, Land, Valaar, Rock, Mountain }`, `ALL: [CellType; 5]`, `passable()`, `conductivity()`, `fallback_rgb()`.
- `world::World<S>` — pub `space`, `valaar: Field`, `params`; `new(space, params)`; `add_source`.
- `field::Field` — `zeros`, `get`, `set`, `add`, `total`, `len`.
- `organism::{Organism, TraitOrganism}` — `TraitOrganism{ genome, pos, energy, age }`, `max_energy(eco)`; `genome.{size, diet, speed}` in `[0,1]`.
- `genome::Genome` — `from_array([f32; 8])` order `[size, valaar_efficiency, speed, diet, repro_threshold, lifespan, heat_tolerance, drought_tolerance]`.
- `population::Population` — `organisms() -> &[TraitOrganism]`, `organisms_mut()`, `spawn`, `len`, `Population::new()`.
- `sim::Sim<S>` — `step()`; pub `world`, `pop`, `rng`, `eco`, `calendar`; `season()`, `population()`, `calendar.{year(), craw()}`.
- `mapsim` — `downscale`, `world_from_materials`, `seed_on_fed_land`, `dot_color` (being replaced).
- `viewer` — `Camera { cx, cy, zoom }` with `pan_pixels`, `zoom_at`, `screen_to_cell`; `render_to_buffer(map, space, layer, cam, w, h, buf)`; `CELL_PX`, `BACKDROP`. std-only.
- `tilemap` — `parse_tmx`, `material_grid(&map, &atlas)`, `Atlas`, `TileMap { width, height, .. }`, `render_tiles_to_buffer(&map, &atlas, &cam, w, h, buf)`.
- `bin/mapview.rs` (current minifb shell, being rewritten) — `TileSim { map, atlas, sim, mats, running }`, `Scene::{Tiles, Terrain}`, `load_scene`, `framed_camera`, `SIM_SCALE = 3`, `SEED_COUNT = 1500`, `WARM_STEPS = 150`.

## Global Constraints

- **Language:** Rust, edition 2021.
- **Dependency change:** remove `minifb`, add `eframe` (used **only** by `bin/mapview.rs`). The engine modules **and `src/viewer.rs`** must not `use eframe`/`use egui` — egui types appear only in the binary. `serde`/`serde_json` stay scoped to `terrain`/`tilemap` as before.
- **egui API drift:** pin a recent `eframe` (this plan targets `eframe = "0.29"`). egui's API moves between minor versions — if a newer release is used, consult `https://docs.rs/eframe` / `https://docs.rs/egui` and adjust the few touchpoints this plan uses: the `run_native` app-creation closure return type, `ColorImage`/`TextureHandle` construction, and scroll-input access. The plan flags each such site.
- **Determinism:** no wall-clock or RNG in rendering/stats. `label_continents` is pure over `(mats, sw, sh)` with a fixed scan order; `compute_stats` is pure over the population; `move_organisms` draws the RNG once per organism before the terrain check, preserving reproducibility.
- **Formatting / lint gate (repo convention):** the repo is **not** rustfmt-clean — **do NOT run `cargo fmt`**. Hand-format to match surrounding code (4-space indent, `//!`/`///` docs). Gate every task on `cargo test` and `cargo clippy --all-targets`.
- **Testing reality:** all *logic* (movement, continents, stats, marker colour) lives in lib modules with unit tests. The `eframe` app is UI and is **not** unit-tested — it is verified by `cargo build --bin mapview` + `cargo clippy` + a manual run. (Per the user, viewer unit tests are not a priority.)
- **Spatial access stays behind `Space`:** sim-resolution material/label/passability arrays are plane-sized (`sw*sh`), indexed `y*sw + x`.
- **Version control:** work on `main` (no feature branch, per user request — small, manually revertible). Commit per task, plain message, **no `Co-Authored-By` trailer**. Confirm with the user before the first commit if unsure whether they want commits vs an uncommitted diff.

---

## Design assumptions (flagged for veto)

- **A1 — Valaar is impassable.** `CellType::Valaar.passable() == false` (joins `Ocean`, `Mountain`). Life lives on the banks and absorbs valaar that diffuses onto adjacent land. Conductivity unchanged.
- **A2 — Passability mask on `World`** as `Option<Vec<bool>>` sized to `space.len()`. `None` = everywhere passable (headless `life`/`sim` and existing tests unchanged). Surface block from the material grid; underground block all-passable.
- **A3 — Diffusion stays terrain-blind** (valaar still bleeds into ocean in the field); irrelevant to what's shown since movement refuses impassable cells. Deferred to plan 5.
- **A4 — Organisms = filled circles.** Radius scales with `genome.size` and zoom; fill colour by `genome.diet` (green autotroph → red predator); a 1-px darker ring behind gives contrast on any terrain. (No glyphs/ASCII.)
- **A5 — Continent = connected component of non-Ocean terrain** (4-connectivity) on the sim grid. Ocean separates continents; mountains and valaar rivers are intra-continent. Computed once at load.
- **A6 — egui viewer.** `eframe::App`; terrain CPU-rendered into a `Vec<u32>` (reusing `render_tiles_to_buffer` / `render_to_buffer`), uploaded as a texture each frame, drawn with the existing `Camera` pan/zoom; organisms via `Painter` circles; right `SidePanel` with stats + `[Pause]`/`[Reseed]` (Tiles scene) and a layer-toggle button (Terrain scene). WASM build deferred.

---

## File Structure

- `src/terrain.rs` — **modified**: `Valaar` impassable; update two tests.
- `src/world.rs` — **modified**: `passability` field + accessors + test.
- `src/ecology.rs` — **modified**: `move_organisms` gains `passable: Option<&[bool]>`; update two tests; add two.
- `src/sim.rs` — **modified**: pass `self.world.passability()`.
- `src/mapsim.rs` — **modified**: `world_from_materials` sets mask; add `label_continents`, `Stats`, `compute_stats`; replace `dot_color` with `marker_color`.
- `src/viewer.rs` — **unchanged** (still produces `Vec<u32>`; stays std-only).
- `Cargo.toml` — **modified**: remove `minifb`, add `eframe`.
- `src/bin/mapview.rs` — **rewritten**: `eframe::App` (terrain texture + pan/zoom + organism circles + stats panel + controls).
- `README.md` — **modified**: note the egui viewer, circle organisms, terrain barriers, stats.

---

### Task 1: Make `Valaar` impassable

**Files:**
- Modify: `src/terrain.rs` (`passable` + two tests)

**Interfaces:**
- Produces: `CellType::passable(self) -> bool` returns `false` for `Ocean | Mountain | Valaar`.

- [ ] **Step 1: Update the two affected tests**

In `src/terrain.rs`, replace `oceans_and_mountains_block_movement_and_valaar`:

```rust
    #[test]
    fn ocean_mountain_and_valaar_block_movement() {
        assert!(!CellType::Ocean.passable());
        assert!(!CellType::Mountain.passable());
        assert!(!CellType::Valaar.passable());
        assert_eq!(CellType::Ocean.conductivity(), 0.0);
        assert_eq!(CellType::Mountain.conductivity(), 0.0);
    }
```

And replace `valaar_conducts_better_than_land`:

```rust
    #[test]
    fn valaar_conducts_better_than_land() {
        assert!(CellType::Valaar.conductivity() > CellType::Land.conductivity());
        assert!(CellType::Land.passable());
        assert!(!CellType::Valaar.passable()); // rivers/reservoir: life lives on the banks
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib terrain`
Expected: FAIL — `assert!(!CellType::Valaar.passable())` panics.

- [ ] **Step 3: Make `Valaar` impassable**

In `src/terrain.rs`:

```rust
    /// Whether an organism may move into this cell (plan 5 consumes this).
    pub fn passable(self) -> bool {
        !matches!(self, CellType::Ocean | CellType::Mountain | CellType::Valaar)
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib terrain && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/terrain.rs
git commit -m "feat: make Valaar terrain impassable"
```

---

### Task 2: Passability mask on `World`

**Files:**
- Modify: `src/world.rs` (field + accessors + test)

**Interfaces:**
- Produces: `World::set_passability(&mut self, mask: Vec<bool>)` (debug-asserts `len == space.len()`); `World::passability(&self) -> Option<&[bool]>` (`None` until set).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/world.rs`:

```rust
    #[test]
    fn passability_defaults_none_and_round_trips() {
        let space = Grid2p5D::new(2, 2);
        let mut world = World::new(space, Params::default());
        assert!(world.passability().is_none());
        let mask = vec![true; world.space.len()];
        world.set_passability(mask);
        assert_eq!(world.passability().unwrap().len(), world.space.len());
        assert!(world.passability().unwrap().iter().all(|&p| p));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib world`
Expected: FAIL — `no method named passability`.

- [ ] **Step 3: Add the field and accessors**

In `src/world.rs`, add to `struct World<S>` (after `access_points`):

```rust
    sources: Vec<Coord>,
    access_points: Vec<(u32, u32)>,
    /// Per-cell passability (sized to `space.len()`); `None` = everywhere
    /// passable. Set from the terrain map; consumed by `ecology::move_organisms`.
    passability: Option<Vec<bool>>,
```

In `World::new`, add `passability: None,` to the struct literal. Add accessors in the first `impl<S: Space> World<S>` block:

```rust
    /// Install a per-cell passability mask (`true` = passable). Length must
    /// equal `space.len()` (all layers, `Space::index` order).
    pub fn set_passability(&mut self, mask: Vec<bool>) {
        debug_assert_eq!(mask.len(), self.space.len(), "mask must cover every cell");
        self.passability = Some(mask);
    }

    /// The passability mask, if one was installed.
    pub fn passability(&self) -> Option<&[bool]> {
        self.passability.as_deref()
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib world && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/world.rs
git commit -m "feat: store an optional passability mask on World"
```

---

### Task 3: Terrain-aware `move_organisms`

**Files:**
- Modify: `src/ecology.rs` (signature + body; update two tests; add two)
- Modify: `src/sim.rs` (pass the mask)

**Interfaces:**
- Consumes: `World::passability` (Task 2).
- Produces: `move_organisms<S: Space>(space, field, pop, eco, rng, passable: Option<&[bool]>)`; `Some(mask)` skips neighbours where `!mask[index]`.

- [ ] **Step 1: Update the two existing move tests**

In `src/ecology.rs`, in `moves_uphill_toward_richer_valaar` and `at_local_max_it_stays_put`, change the call to pass `None`:

```rust
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, None);
```

- [ ] **Step 2: Add the two new behaviour tests**

Add to the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn does_not_step_onto_impassable_richer_neighbor() {
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        for x in 0..4u32 {
            field.set(space.index(Coord::new(x, 0, Layer::Surface)), x as f32);
        }
        let mut mask = vec![true; space.len()];
        mask[space.index(Coord::new(3, 0, Layer::Surface))] = false; // richest cell barred
        let start = Coord::new(2, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5]), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&mask));
        assert_eq!(pop.organisms()[0].pos, start, "must not enter an impassable cell");
    }

    #[test]
    fn boxed_in_organism_stays_and_pays_nothing() {
        let space = Grid2p5D::new(3, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        for x in 0..3u32 {
            field.set(space.index(Coord::new(x, 0, Layer::Surface)), x as f32);
        }
        let mut mask = vec![true; space.len()];
        mask[space.index(Coord::new(0, 0, Layer::Surface))] = false;
        mask[space.index(Coord::new(2, 0, Layer::Surface))] = false;
        let center = Coord::new(1, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5]), center, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&mask));
        assert_eq!(pop.organisms()[0].pos, center);
        assert_eq!(pop.organisms()[0].energy, 5.0, "no move, no cost");
    }
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: FAIL — `move_organisms` arity mismatch (compile error).

- [ ] **Step 4: Update `move_organisms`**

In `src/ecology.rs`, replace the whole function:

```rust
/// Each organism moves with probability `speed` toward its richest in-bounds,
/// **passable** planar neighbour (gradient ascent on valaar). Moving costs
/// `move_cost·speed`. `passable`: `None` = no terrain constraint; `Some(mask)`
/// (sized to `space.len()`) bars stepping into cells where `mask[index]` is
/// false. Neighbours never cross layers, so organisms stay on their layer.
pub fn move_organisms<S: Space>(
    space: &S,
    field: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
    passable: Option<&[bool]>,
) {
    for o in pop.organisms_mut() {
        // Draw first so the rng stream advances once per organism regardless.
        if rng.next_unit() >= o.genome.speed {
            continue;
        }
        let mut best = o.pos;
        let mut best_v = field.get(space.index(o.pos));
        for n in space.planar_neighbors(o.pos) {
            let ni = space.index(n);
            if let Some(mask) = passable {
                if !mask[ni] {
                    continue; // impassable terrain blocks the step
                }
            }
            let v = field.get(ni);
            if v > best_v {
                best_v = v;
                best = n;
            }
        }
        if best != o.pos {
            o.pos = best;
            o.energy -= eco.move_cost * o.genome.speed;
        }
    }
}
```

- [ ] **Step 5: Wire the mask through `Sim::step`**

In `src/sim.rs`, update the `move_organisms` call:

```rust
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
            self.world.passability(),
        );
```

- [ ] **Step 6: Run the full suite**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 7: Commit**

```bash
git add src/ecology.rs src/sim.rs
git commit -m "feat: gate organism movement on terrain passability"
```

---

### Task 4: Feed the passability mask from the material map

**Files:**
- Modify: `src/mapsim.rs` (`world_from_materials` + a test)

**Interfaces:**
- Consumes: `CellType::passable` (Task 1), `World::set_passability` (Task 2).
- Produces: `world_from_materials` returns a `World` whose `passability()` is `Some` (surface block from `mats`, underground all-passable).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/mapsim.rs` (helper `grid` maps `'O'→Ocean`, `'V'→Valaar`, else `Land`):

```rust
    #[test]
    fn world_marks_ocean_and_valaar_impassable() {
        let (w, h, m) = grid(&["OVL"]); // ocean, valaar, land
        let world = world_from_materials(w, h, &m);
        let mask = world.passability().expect("mask installed");
        let idx = |x: u32| world.space.index(Coord::new(x, 0, Layer::Surface));
        assert!(!mask[idx(0)], "ocean impassable");
        assert!(!mask[idx(1)], "valaar impassable");
        assert!(mask[idx(2)], "land passable");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib mapsim`
Expected: FAIL — `mask installed` panics (None).

- [ ] **Step 3: Build and install the mask**

In `src/mapsim.rs`, replace the body of `world_from_materials`:

```rust
/// A `World` on the sim grid with every `Valaar` cell registered as a surface
/// source — so valaar flows out of the drawn reservoir and rivers — and a
/// passability mask derived from the materials (surface block from `mats`,
/// underground block all-passable).
pub fn world_from_materials(sw: u32, sh: u32, mats: &[CellType]) -> World<Grid2p5D> {
    let mut world = World::new(Grid2p5D::new(sw, sh), Params::default());
    for y in 0..sh {
        for x in 0..sw {
            if mats[(y * sw + x) as usize] == CellType::Valaar {
                world.add_source(Coord::new(x, y, Layer::Surface));
            }
        }
    }
    let plane = (sw * sh) as usize;
    let mut mask = vec![true; world.space.len()]; // both layers; underground all-passable
    for i in 0..plane {
        mask[i] = mats[i].passable(); // surface block occupies indices 0..plane
    }
    world.set_passability(mask);
    world
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib mapsim && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/mapsim.rs
git commit -m "feat: derive World passability from the material map"
```

---

### Task 5: Continents + population statistics

**Files:**
- Modify: `src/mapsim.rs` (`label_continents`, `Stats`, `compute_stats` + tests)

**Interfaces:**
- Produces:
  - `label_continents(mats: &[CellType], sw: u32, sh: u32) -> (Vec<Option<u32>>, u32)` — per-cell continent label (`None` for Ocean) + count; 4-connectivity over non-Ocean; deterministic row-major scan.
  - `pub struct Stats { total, autotrophs, predators, mean_size, continents: Vec<(u32, usize)> }` (continents sorted by count desc, then label asc).
  - `compute_stats(pop: &Population, sw: u32, labels: &[Option<u32>], n_continents: u32) -> Stats`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/mapsim.rs`:

```rust
    #[test]
    fn continents_split_on_ocean_only() {
        let (w, h, m) = grid(&["LOLL"]); // Land | Ocean | Land Land
        let (labels, n) = label_continents(&m, w, h);
        assert_eq!(n, 2);
        assert_eq!(labels[0], Some(0));
        assert_eq!(labels[1], None); // ocean
        assert_eq!(labels[2], Some(1));
        assert_eq!(labels[3], Some(1));
    }

    #[test]
    fn compute_stats_tallies_diet_size_and_continents() {
        let (w, h, m) = grid(&["LOLL"]);
        let (labels, n) = label_continents(&m, w, h);
        let mut pop = Population::new();
        let g = |diet: f32, size: f32| {
            Genome::from_array([size, 0.5, 0.5, diet, 0.5, 0.5, 0.5, 0.5])
        };
        pop.spawn(TraitOrganism::new(g(0.0, 0.2), Coord::new(0, 0, Layer::Surface), 1.0));
        pop.spawn(TraitOrganism::new(g(0.0, 0.4), Coord::new(0, 0, Layer::Surface), 1.0));
        pop.spawn(TraitOrganism::new(g(1.0, 0.6), Coord::new(2, 0, Layer::Surface), 1.0));

        let s = compute_stats(&pop, w, &labels, n);
        assert_eq!(s.total, 3);
        assert_eq!(s.autotrophs, 2);
        assert_eq!(s.predators, 1);
        assert!((s.mean_size - 0.4).abs() < 1e-6);
        assert_eq!(s.continents, vec![(0, 2), (1, 1)]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib mapsim`
Expected: FAIL — `cannot find function label_continents` / `compute_stats`.

- [ ] **Step 3: Implement continents + stats**

In `src/mapsim.rs`, add (and add `use crate::population::Population;` at the top if not already imported):

```rust
/// Flood-fill connected components of non-Ocean terrain (4-connectivity) on the
/// `sw×sh` sim plane. Returns a per-cell continent label (`None` for Ocean) and
/// the number of continents. Deterministic: row-major scan, first-seen labels.
pub fn label_continents(mats: &[CellType], sw: u32, sh: u32) -> (Vec<Option<u32>>, u32) {
    let n = (sw * sh) as usize;
    let mut labels: Vec<Option<u32>> = vec![None; n];
    let mut next: u32 = 0;
    for start in 0..n {
        if mats[start] == CellType::Ocean || labels[start].is_some() {
            continue;
        }
        let label = next;
        next += 1;
        labels[start] = Some(label);
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let (x, y) = (i as u32 % sw, i as u32 / sw);
            let mut neighbors = Vec::with_capacity(4);
            if x > 0 {
                neighbors.push(i - 1);
            }
            if x + 1 < sw {
                neighbors.push(i + 1);
            }
            if y > 0 {
                neighbors.push(i - sw as usize);
            }
            if y + 1 < sh {
                neighbors.push(i + sw as usize);
            }
            for j in neighbors {
                if mats[j] != CellType::Ocean && labels[j].is_none() {
                    labels[j] = Some(label);
                    stack.push(j);
                }
            }
        }
    }
    (labels, next)
}

/// A snapshot of the living population for the viewer HUD.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub total: usize,
    pub autotrophs: usize,
    pub predators: usize,
    pub mean_size: f32,
    /// `(continent_label, population)`, sorted by population desc then label asc.
    pub continents: Vec<(u32, usize)>,
}

/// Tally the population: diet split (`diet <= 0.5` = autotroph-leaning), mean
/// body size, and per-continent population using `labels` from
/// `label_continents`. `sw` is the sim-plane width (to index `labels`).
pub fn compute_stats(
    pop: &Population,
    sw: u32,
    labels: &[Option<u32>],
    n_continents: u32,
) -> Stats {
    let orgs = pop.organisms();
    let total = orgs.len();
    let mut autotrophs = 0usize;
    let mut size_sum = 0.0f32;
    let mut per = vec![0usize; n_continents as usize];
    for o in orgs {
        if o.genome.diet <= 0.5 {
            autotrophs += 1;
        }
        size_sum += o.genome.size;
        let idx = (o.pos.y * sw + o.pos.x) as usize;
        if let Some(Some(label)) = labels.get(idx) {
            per[*label as usize] += 1;
        }
    }
    let mean_size = if total > 0 { size_sum / total as f32 } else { 0.0 };
    let mut continents: Vec<(u32, usize)> =
        per.iter().enumerate().map(|(l, &c)| (l as u32, c)).collect();
    continents.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Stats { total, autotrophs, predators: total - autotrophs, mean_size, continents }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib mapsim && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/mapsim.rs
git commit -m "feat: continent labelling + population statistics"
```

---

### Task 6: Diet-coloured organism markers

**Files:**
- Modify: `src/mapsim.rs` (replace `dot_color` with `marker_color`; test)

**Interfaces:**
- Produces: `marker_color(o: &TraitOrganism) -> u32` — packed `0x00RRGGBB`, green autotroph → red predator. `dot_color` removed.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/mapsim.rs`:

```rust
    #[test]
    fn marker_color_greens_autotrophs_reds_predators() {
        let c = Coord::new(0, 0, Layer::Surface);
        let auto = TraitOrganism::new(
            Genome::from_array([0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.5]), c, 1.0);
        let pred = TraitOrganism::new(
            Genome::from_array([0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 0.5, 0.5]), c, 1.0);
        let green = |p: u32| (p >> 8) & 0xFF;
        let red = |p: u32| (p >> 16) & 0xFF;
        assert!(green(marker_color(&auto)) > red(marker_color(&auto)));
        assert!(red(marker_color(&pred)) > green(marker_color(&pred)));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib mapsim`
Expected: FAIL — `cannot find function marker_color`.

- [ ] **Step 3: Replace `dot_color`**

In `src/mapsim.rs`, delete `dot_color` and add:

```rust
/// Packed `0x00RRGGBB` marker colour: hue by diet (green autotroph → red
/// predator). Brightness is constant — the circle outline carries contrast.
pub fn marker_color(o: &TraitOrganism) -> u32 {
    let r = (o.genome.diet * 255.0) as u32;
    let g = ((1.0 - o.genome.diet) * 255.0) as u32;
    let b = 60u32;
    (r << 16) | (g << 8) | b
}
```

If removing `dot_color` leaves `use crate::params::EcoParams;` unused, drop that import (clippy will flag it). It is still needed if `seed_on_fed_land` references `EcoParams`; only remove if clippy says it's unused.

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib mapsim && cargo clippy --all-targets`
Expected: PASS, clippy clean. (The `mapview` binary will not build until Task 8 — use `cargo test --lib` here.)

- [ ] **Step 5: Commit**

```bash
git add src/mapsim.rs
git commit -m "feat: diet-coloured organism markers (replace dot_color)"
```

---

### Task 7: Swap the window dependency to eframe + minimal app

**Files:**
- Modify: `Cargo.toml` (remove `minifb`, add `eframe`)
- Rewrite: `src/bin/mapview.rs` (eframe skeleton: terrain texture + pan/zoom; no organisms/HUD yet)

**Interfaces:**
- Consumes: `viewer::{Camera, render_to_buffer}`, `tilemap::render_tiles_to_buffer`, existing `load_scene`/`framed_camera`/`TileSim`/`Scene`.
- Produces: a runnable eframe window showing the terrain with drag-pan and scroll-zoom.

> No unit test (UI). Verified by build + clippy + manual run.

- [ ] **Step 1: Swap the dependency**

In `Cargo.toml`, under `[dependencies]`, remove the `minifb` line and add:

```toml
eframe = "0.29"
```

(If a newer eframe is selected, keep the API-drift note in mind for Steps below.)

- [ ] **Step 2: Rewrite `bin/mapview.rs` — skeleton**

Replace the entire contents of `src/bin/mapview.rs` with the following. It keeps `load_scene`, `framed_camera`, `TileSim`, and `Scene` (carried over from the current file, adjusted), and renders terrain into a `Vec<u32>` each frame which is uploaded as an egui texture.

```rust
//! Interactive map viewer + live simulation, on egui/eframe.
//!   drag = pan, scroll = zoom.
//!   .tmx map: textured, with a running sim (Pause/Reseed in the side panel).
//!   .json map: a `TerrainMap` in solid `CellType` colours (layer toggle button).
//! Usage: `cargo run --release --bin mapview [map]` (default alife_map_blended.tmx).

use alife::mapsim::{downscale, label_continents, seed_on_fed_land, world_from_materials};
use alife::params::EcoParams;
use alife::sim::Sim;
use alife::space::{Grid2p5D, Layer, Space};
use alife::terrain::{load_json, CellType, TerrainMap};
use alife::tilemap::{material_grid, parse_tmx, render_tiles_to_buffer, Atlas, TileMap};
use alife::viewer::{render_to_buffer, Camera};
use std::path::Path;

const SIM_SCALE: u32 = 3; // sim grid is the display grid downscaled by this
const SEED_COUNT: usize = 1500;
const WARM_STEPS: usize = 150; // spread valaar before seeding life

/// A live tile simulation: the textured map, its atlas, a sim on a downscaled
/// material grid, and cached continent labels for the stats panel.
struct TileSim {
    map: TileMap,
    atlas: Atlas,
    sim: Sim<Grid2p5D>,
    mats: Vec<CellType>,          // sim-resolution materials
    continents: Vec<Option<u32>>, // per sim-cell continent label
    n_continents: u32,
    running: bool,
}

impl TileSim {
    fn reseed(&mut self) {
        self.sim.pop = alife::population::Population::new();
        seed_on_fed_land(&mut self.sim, &self.mats, SEED_COUNT, 1e-3);
    }
}

enum Scene {
    Tiles(TileSim),
    Terrain { map: TerrainMap, space: Grid2p5D, layer: Layer },
}

impl Scene {
    fn dims(&self) -> (u32, u32) {
        match self {
            Scene::Tiles(t) => (t.map.width, t.map.height),
            Scene::Terrain { map, .. } => (map.width(), map.height()),
        }
    }
}

fn load_scene(path: &str) -> Scene {
    if path.ends_with(".tmx") {
        let xml = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let map = parse_tmx(&xml).unwrap_or_else(|e| panic!("parse {path}: {e}"));
        let dir = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
        let stem = Path::new(&map.atlas_image).file_stem().and_then(|s| s.to_str()).unwrap_or("sheet");
        let atlas_path = dir.join(format!("{stem}.rgba"));
        let bytes = std::fs::read(&atlas_path).unwrap_or_else(|e| {
            panic!("read atlas {}: {e}\nBake it first: `python3 tools/png_to_rgba.py`", atlas_path.display())
        });
        let atlas = Atlas::load(&bytes).unwrap_or_else(|e| panic!("atlas {}: {e}", atlas_path.display()));

        let mats = material_grid(&map, &atlas);
        let (sw, sh, sim_mats) = downscale(&mats, map.width, map.height, SIM_SCALE);
        let (continents, n_continents) = label_continents(&sim_mats, sw, sh);
        let world = world_from_materials(sw, sh, &sim_mats);
        let mut sim = Sim::new(world, EcoParams::default(), 0xA11FE);
        for _ in 0..WARM_STEPS {
            sim.world.step();
        }
        let mut t = TileSim {
            map,
            atlas,
            sim,
            mats: sim_mats,
            continents,
            n_continents,
            running: true,
        };
        t.reseed();
        Scene::Tiles(t)
    } else {
        let map = load_json(Path::new(path)).unwrap_or_else(|e| {
            panic!("load map {path}: {e}\nBuild one first, e.g. `python3 tools/sketch_to_map.py <sketch.png>`.")
        });
        let space = Grid2p5D::new(map.width(), map.height());
        Scene::Terrain { map, space, layer: Layer::Surface }
    }
}

/// Fit the whole map in a `vw×vh` viewport and centre it.
fn framed_camera(mw: u32, mh: u32, vw: usize, vh: usize) -> Camera {
    let zoom = (vw as f32 / mw as f32).min(vh as f32 / mh as f32).max(0.25);
    Camera {
        cx: mw as f32 / 2.0 - (vw as f32 / zoom) / 2.0,
        cy: mh as f32 / 2.0 - (vh as f32 / zoom) / 2.0,
        zoom,
    }
}

/// CPU-render the current scene's terrain into `buf` (sized `vw*vh`) under `cam`.
fn render_terrain(scene: &Scene, cam: &Camera, vw: u32, vh: u32, buf: &mut [u32]) {
    match scene {
        Scene::Tiles(t) => {
            // Half-tile offset: autotiled tiles are a dual grid; shift +½ cell.
            let tile_cam = Camera { cx: cam.cx + 0.5, cy: cam.cy + 0.5, zoom: cam.zoom };
            render_tiles_to_buffer(&t.map, &t.atlas, &tile_cam, vw, vh, buf);
        }
        Scene::Terrain { map, space, layer } => {
            render_to_buffer(map, space, *layer, cam, vw, vh, buf);
        }
    }
}

/// Convert a `0x00RRGGBB` buffer into an egui image.
fn to_color_image(buf: &[u32], w: usize, h: usize) -> egui::ColorImage {
    let pixels = buf
        .iter()
        .map(|&p| egui::Color32::from_rgb((p >> 16) as u8, (p >> 8) as u8, p as u8))
        .collect();
    egui::ColorImage { size: [w, h], pixels }
}

struct MapApp {
    scene: Scene,
    cam: Camera,
    framed: bool,
    buf: Vec<u32>,
    tex: Option<egui::TextureHandle>,
}

impl MapApp {
    fn new(path: &str) -> Self {
        let scene = load_scene(path);
        MapApp { scene, cam: Camera::new(), framed: false, buf: Vec::new(), tex: None }
    }
}

impl eframe::App for MapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Advance the live sim each frame (Tiles scene only, when running).
        if let Scene::Tiles(t) = &mut self.scene {
            if t.running {
                t.sim.step();
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let (vw, vh) = (avail.x.max(1.0) as u32, avail.y.max(1.0) as u32);

            // Frame the map the first time we know the viewport size.
            if !self.framed {
                let (mw, mh) = self.scene.dims();
                self.cam = framed_camera(mw, mh, vw as usize, vh as usize);
                self.framed = true;
            }

            // CPU-render terrain into our buffer and upload it as a texture.
            let len = (vw * vh) as usize;
            if self.buf.len() != len {
                self.buf = vec![0u32; len];
            }
            render_terrain(&self.scene, &self.cam, vw, vh, &mut self.buf);
            let img = to_color_image(&self.buf, vw as usize, vh as usize);
            let tex = self
                .tex
                .get_or_insert_with(|| ui.ctx().load_texture("terrain", img.clone(), egui::TextureOptions::NEAREST));
            tex.set(img, egui::TextureOptions::NEAREST);

            // Draw the terrain to fill the panel and capture pan/zoom input.
            let (rect, response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
            ui.painter().image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            if response.dragged() {
                let d = response.drag_delta();
                self.cam.pan_pixels(d.x, d.y);
            }
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(pos) = response.hover_pos() {
                    let local = pos - rect.min;
                    self.cam.zoom_at(local.x, local.y, if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 });
                }
            }
        });

        ctx.request_repaint(); // keep animating the live sim
    }
}

fn main() -> eframe::Result {
    let path = std::env::args().nth(1).unwrap_or_else(|| "assets/alife_map_blended.tmx".into());
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "alife — map viewer",
        native_options,
        Box::new(move |_cc| Ok(Box::new(MapApp::new(&path)))),
    )
}
```

> **API-drift checkpoints (verify against the eframe version you pinned):** the `run_native` creation closure returns `Result<Box<dyn eframe::App>, _>` here via `Ok(Box::new(..))`; `egui::ColorImage { size, pixels }`; `ui.input(|i| i.raw_scroll_delta.y)`; `TextureOptions::NEAREST`; `eframe::Result` as the `main` return. If your version differs, adjust these per its docs.

- [ ] **Step 3: Build, lint, run**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: builds clean, clippy clean. (If clippy warns about `clone()` on the first texture load, that's fine to leave or simplify.)

Manual check: `cargo run --release --bin mapview` opens a window showing the textured map; drag pans, scroll zooms. (`cargo run --release --bin mapview out/alchaea.json` shows the solid-colour terrain, if you have one.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/bin/mapview.rs
git commit -m "feat: rewrite the map viewer on egui/eframe (terrain + pan/zoom)"
```

---

### Task 8: Draw organisms as circles

**Files:**
- Modify: `src/bin/mapview.rs` (organism overlay)

**Interfaces:**
- Consumes: `mapsim::marker_color`, `Sim`/`Population`, the `rect`/`cam` from Task 7.
- Produces: organisms drawn as outlined circles sized by body, only on the Tiles scene.

> No unit test (UI). Verified by build + clippy + manual run.

- [ ] **Step 1: Add the organism-drawing helper**

In `src/bin/mapview.rs`, add `use alife::mapsim::marker_color;` to the `mapsim` import line, and add this free function:

```rust
/// Draw each organism as an outlined circle inside `rect`: radius scales with
/// body size and zoom, fill by diet, with a darker ring behind for contrast.
fn draw_organisms(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim) {
    let cell_px = cam.zoom * SIM_SCALE as f32;
    for o in t.sim.pop.organisms() {
        let wx = (o.pos.x * SIM_SCALE + SIM_SCALE / 2) as f32;
        let wy = (o.pos.y * SIM_SCALE + SIM_SCALE / 2) as f32;
        let sx = rect.min.x + (wx - cam.cx) * cam.zoom;
        let sy = rect.min.y + (wy - cam.cy) * cam.zoom;
        let center = egui::pos2(sx, sy);
        if !rect.contains(center) {
            continue;
        }
        let radius = ((0.25 + 0.5 * o.genome.size) * cell_px * 0.5).clamp(1.5, 64.0);
        let c = marker_color(o);
        let fill = egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        painter.circle_filled(center, radius + 1.0, egui::Color32::from_black_alpha(220)); // ring
        painter.circle_filled(center, radius, fill);
    }
}
```

- [ ] **Step 2: Call it after drawing the terrain**

In `MapApp::update`, inside the `CentralPanel` closure, **after** the pan/zoom input block, add:

```rust
            if let Scene::Tiles(t) = &self.scene {
                let painter = ui.painter_at(rect); // clip organisms to the map rect
                draw_organisms(&painter, rect, &self.cam, t);
            }
```

- [ ] **Step 3: Build, lint, run**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: builds clean, clippy clean.

Manual check: `cargo run --release --bin mapview` — organisms appear as outlined circles, clearly visible on land; bigger bodies are bigger circles; green autotrophs vs red predators are distinguishable; zoom in and confirm none sit on the sea or on the valaar rivers/reservoir.

- [ ] **Step 4: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: draw organisms as outlined circles sized by body"
```

---

### Task 9: Stats side panel + controls

**Files:**
- Modify: `src/bin/mapview.rs` (right `SidePanel`: stats + Pause/Reseed; layer toggle for Terrain scene)

**Interfaces:**
- Consumes: `mapsim::compute_stats`, `TileSim.{continents, n_continents}`, `Sim`.
- Produces: a live stats panel and controls.

> No unit test (UI). Verified by build + clippy + manual run.

- [ ] **Step 1: Import `compute_stats`**

In `src/bin/mapview.rs`, add `compute_stats` to the `mapsim` import line:

```rust
use alife::mapsim::{
    compute_stats, downscale, label_continents, marker_color, seed_on_fed_land,
    world_from_materials,
};
```

- [ ] **Step 2: Add the side panel**

In `MapApp::update`, **before** the `CentralPanel`, add:

```rust
        egui::SidePanel::right("stats").default_width(220.0).show(ctx, |ui| {
            ui.heading("alife");
            match &mut self.scene {
                Scene::Tiles(t) => {
                    let sw = t.sim.world.space.width();
                    let s = compute_stats(&t.sim.pop, sw, &t.continents, t.n_continents);
                    ui.label(format!("season {:?}", t.sim.season()));
                    ui.label(format!("year {}  craw {}", t.sim.calendar.year(), t.sim.calendar.craw()));
                    ui.separator();
                    ui.label(format!("population  {}", s.total));
                    ui.label(format!("autotrophs  {}", s.autotrophs));
                    ui.label(format!("predators   {}", s.predators));
                    ui.label(format!("mean size   {:.2}", s.mean_size));
                    ui.separator();
                    ui.label(format!("continents: {}", t.n_continents));
                    for (label, count) in s.continents.iter().filter(|(_, c)| *c > 0).take(8) {
                        ui.label(format!("  c{label}: {count}"));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(if t.running { "Pause" } else { "Resume" }).clicked() {
                            t.running = !t.running;
                        }
                        if ui.button("Reseed").clicked() {
                            t.reseed();
                        }
                    });
                }
                Scene::Terrain { layer, .. } => {
                    ui.label(format!("layer: {:?}", layer));
                    if ui.button("Toggle layer").clicked() {
                        *layer = match layer {
                            Layer::Surface => Layer::Underground,
                            Layer::Underground => Layer::Surface,
                        };
                    }
                }
            }
        });
```

- [ ] **Step 3: Build, lint, run**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: builds clean, clippy clean.

Manual check: `cargo run --release --bin mapview` — the right panel shows population, autotroph/predator split, mean size, and per-continent counts, updating live; **Pause/Resume** halts/continues the sim; **Reseed** repopulates. With a `.json` map, the panel shows a working layer toggle.

- [ ] **Step 4: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: stats side panel with Pause/Reseed controls"
```

---

### Task 10: Document the new viewer

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the dependency note and the mapview description**

In `README.md`:
- In the design-principles dependency note, change the viewer dependency from `minifb` to `eframe`/`egui` (the engine + `viewer` core stay std-only; egui is in the binary only).
- In "Running it", update the `mapview` description to:

```markdown
# Open the interactive map viewer (egui): drag to pan, scroll to zoom.
#   .tmx -> textured live sim; organisms are circles sized by body and coloured
#           by diet (green autotroph -> red predator) and stay off ocean/valaar.
#           Right panel: population, diet split, mean size, per-continent counts,
#           and Pause/Reseed.
#   .json -> TerrainMap in solid CellType colours (layer toggle in the panel).
cargo run --release --bin mapview [map.tmx | map.json]
```

- [ ] **Step 2: Lint check and commit**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

```bash
git add README.md
git commit -m "docs: egui viewer, circle organisms, terrain barriers, stats"
```

---

## Self-Review

**Spec coverage:**
- Organisms hard to distinguish → Tasks 6/8 (diet colour, body-sized circles, outline ring). ✓
- Organisms walk onto Ocean/Valaar → Tasks 1–4 (Valaar impassable; mask on `World`; movement gate; mask from materials). ✓
- Statistics by diet + location → Task 5 (`compute_stats`), Task 9 (panel). ✓
- Colonies by continent → Task 5 `label_continents` (non-Ocean components), Task 9 display. ✓
- egui rewrite, WASM-ready, circles, stats + Pause/Reseed → Tasks 7–9. ✓
- No `cargo fmt`; no `Co-Authored-By`; work on `main`; egui only in the binary → Global Constraints. ✓

**Placeholder scan:** every code step shows full code; run steps show command + expected result; egui API-drift points are explicitly flagged rather than hand-waved. ✓

**Type consistency:** `passable: Option<&[bool]>` (Tasks 2/3/4); `World::{set_passability, passability}` consistent; `marker_color(&TraitOrganism) -> u32` used in Tasks 6/8; `label_continents -> (Vec<Option<u32>>, u32)` + `compute_stats(pop, sw, labels, n)` consistent across Tasks 5/9; `Stats.continents: Vec<(u32, usize)>` consumed as such in Task 9; `Camera{cx,cy,zoom}` + `pan_pixels`/`zoom_at` reused unchanged from `viewer`. ✓

**Deferred (not in this plan):** conductivity-weighted diffusion; the WASM/Trunk build of the viewer; population-over-time charts and parameter sliders.
