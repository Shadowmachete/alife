# Artificial-life Sim — World Substrate (2.5D) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic, tested CPU core for the world *substrate* — a 2.5D spatial grid (planar 2D + discrete surface/underground layers) and the **valaar** scalar field that flows from a central source, diffuses, exchanges between layers at access points, and decays — producing an emergent centre-rich → edge-poor gradient.

**Architecture:** A single Rust library crate (`alife`) with a small headless binary. All spatial access goes through a `Space` trait; the only concrete implementation here is `Grid2p5D` (2D planar + 2 discrete layers). Valaar dynamics are pure functions over `(Space, Field)`, composed by `World::step()`. Everything is CPU + deterministic so it is unit-testable; field data is stored as flat `Vec<f32>` arrays so a later plan can move the hot loops to GPU (wgpu) buffers unchanged.

**Tech Stack:** Rust (edition 2021), std only — no external dependencies in this plan.

**Project root:** `~/dev/alife/` (working name — rename freely). All file paths below are relative to it.

**Design source:** `~/dev/ideas/world-design.md` (§F "Working model") and `~/dev/ideas/artificial-life-sim.md`.

## Global Constraints

- **Language:** Rust, edition 2021. **No external crates** in this plan (std only).
- **2D spatial interface (load-bearing):** all spatial access goes through the `Space` trait. The concrete impl is `Grid2p5D` = planar **2D** + a discrete `Layer` (Surface/Underground) — "2.5D". The surface/underground split is a **discrete layer, not a continuous third axis**. `SPATIAL_DIMS == 2`. **No code outside `src/space.rs` may assume a particular dimensionality, neighbourhood, or index layout** — it must call `Space` methods.
- **Future 3D (documented seam):** to go truly 3D later, change `SPATIAL_DIMS` to 3 and add a `Grid3D` implementing `Space`; nothing outside `src/space.rs` should need changes. This intent is recorded in a doc-comment in `src/space.rs`.
- **CPU reference core first:** GPU/wgpu acceleration is a *later* plan. Field data is `Vec<f32>` specifically so it ports to GPU buffers later.
- **Determinism:** no RNG anywhere in this plan. Identical inputs → identical fields, every run.
- **Every dynamics rate is a tunable dial:** they live on `Params` (edge-of-chaos tuning later).
- **Valaar = the single energy currency** (decided). This plan implements only the valaar **field substrate** — no organisms, no heat/water fields, no seasons yet.
- **Version control optional:** the user runs no VCS yet. Each task's final step is a *local checkpoint*; run `git` only if you have initialised a repo.

**Scope (this is plan 1 of several).** Later plans: trait-vector organisms; the ecology/energy loop (absorb → eat → spend → die → recycle); heat & water fields + the 6-arh season cycle (incl. Vraze=crystallisation); mutation-as-a-field; rendering + wgpu acceleration; the web viewer; and the natural-history / species-clustering layer.

---

### Task 1: Crate skeleton + spatial types

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/space.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub const SPATIAL_DIMS: usize`, `pub const LAYER_COUNT: usize`, `pub enum Layer { Surface = 0, Underground = 1 }` with `pub const ALL: [Layer; LAYER_COUNT]`, `pub struct Coord { pub x: u32, pub y: u32, pub layer: Layer }` with `pub fn new(x: u32, y: u32, layer: Layer) -> Coord`.

- [ ] **Step 1: Create the crate manifest and module wiring**

Create `Cargo.toml`:

```toml
[package]
name = "alife"
version = "0.1.0"
edition = "2021"

[dependencies]
```

Create `src/lib.rs`:

```rust
pub mod space;
```

- [ ] **Step 2: Write the failing test**

Create `src/space.rs` with only the test module (the items it references don't exist yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_is_two_dimensional() {
        assert_eq!(SPATIAL_DIMS, 2);
        assert_eq!(LAYER_COUNT, 2);
        assert_eq!(Layer::ALL.len(), LAYER_COUNT);
        assert_eq!(Layer::Surface as usize, 0);
        assert_eq!(Layer::Underground as usize, 1);
    }

    #[test]
    fn coord_constructs() {
        let c = Coord::new(3, 4, Layer::Underground);
        assert_eq!(c.x, 3);
        assert_eq!(c.y, 4);
        assert_eq!(c.layer, Layer::Underground);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find value SPATIAL_DIMS` / `cannot find type Coord` etc.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/space.rs`:

```rust
//! Spatial model for the world.
//!
//! INVARIANT: the world is **2-dimensional** (a planar grid). The vertical axis
//! is a *discrete* `Layer` (Surface / Underground), NOT a continuous third
//! dimension ("2.5D"). All code outside this module MUST talk to space through
//! the `Space` trait so the dimensionality stays swappable.
//!
//! FUTURE (3D): to make the world truly 3D, change `SPATIAL_DIMS` to 3 and add a
//! `Grid3D` implementation of `Space`. Nothing outside this module should need to
//! change.

/// Number of *continuous* spatial dimensions. The world is planar (2D); the
/// surface/underground split is a discrete layer, not a third axis.
pub const SPATIAL_DIMS: usize = 2;

/// How many discrete vertical layers exist.
pub const LAYER_COUNT: usize = 2;

/// Discrete vertical layers (the "2.5D" of the model).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Layer {
    Surface = 0,
    Underground = 1,
}

impl Layer {
    /// All layers, in index order.
    pub const ALL: [Layer; LAYER_COUNT] = [Layer::Surface, Layer::Underground];
}

/// A cell coordinate: planar `(x, y)` plus a discrete `layer`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Coord {
    pub x: u32,
    pub y: u32,
    pub layer: Layer,
}

impl Coord {
    pub fn new(x: u32, y: u32, layer: Layer) -> Self {
        Coord { x, y, layer }
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 2 passed`

- [ ] **Step 6: Checkpoint** (local only — run git only if you initialised a repo)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: crate skeleton + spatial types"
```

---

### Task 2: `Space` trait + `Grid2p5D`

**Files:**
- Modify: `src/space.rs` (add trait + struct above the `tests` module; extend `tests`)

**Interfaces:**
- Consumes: `Coord`, `Layer`, `LAYER_COUNT` (Task 1).
- Produces:
  - `pub trait Space { fn width(&self) -> u32; fn height(&self) -> u32; fn len(&self) -> usize; fn in_bounds(&self, c: Coord) -> bool; fn index(&self, c: Coord) -> usize; fn planar_neighbors(&self, c: Coord) -> Vec<Coord>; }`
  - `pub struct Grid2p5D` with `pub fn new(width: u32, height: u32) -> Grid2p5D`.

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/space.rs`:

```rust
    #[test]
    fn grid_len_counts_all_layers() {
        let g = Grid2p5D::new(4, 3);
        assert_eq!(g.width(), 4);
        assert_eq!(g.height(), 3);
        assert_eq!(g.len(), LAYER_COUNT * 4 * 3);
    }

    #[test]
    fn index_is_unique_per_cell() {
        let g = Grid2p5D::new(4, 3);
        let mut seen = std::collections::HashSet::new();
        for layer in Layer::ALL {
            for y in 0..g.height() {
                for x in 0..g.width() {
                    let i = g.index(Coord::new(x, y, layer));
                    assert!(i < g.len());
                    assert!(seen.insert(i), "index {i} repeated");
                }
            }
        }
        assert_eq!(seen.len(), g.len());
    }

    #[test]
    fn in_bounds_rejects_outside() {
        let g = Grid2p5D::new(4, 3);
        assert!(g.in_bounds(Coord::new(3, 2, Layer::Surface)));
        assert!(!g.in_bounds(Coord::new(4, 2, Layer::Surface)));
        assert!(!g.in_bounds(Coord::new(0, 3, Layer::Underground)));
    }

    #[test]
    fn planar_neighbors_respect_edges_and_layer() {
        let g = Grid2p5D::new(4, 3);
        // interior cell -> 4 neighbours
        assert_eq!(g.planar_neighbors(Coord::new(1, 1, Layer::Surface)).len(), 4);
        // corner -> 2
        assert_eq!(g.planar_neighbors(Coord::new(0, 0, Layer::Surface)).len(), 2);
        // edge -> 3
        assert_eq!(g.planar_neighbors(Coord::new(1, 0, Layer::Surface)).len(), 3);
        // neighbours never cross layers
        for n in g.planar_neighbors(Coord::new(1, 1, Layer::Underground)) {
            assert_eq!(n.layer, Layer::Underground);
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find type Grid2p5D` / `no method named width`.

- [ ] **Step 3: Write the minimal implementation**

Insert above the `tests` module in `src/space.rs`:

```rust
/// The spatial topology the rest of the engine talks to. Implementations decide
/// dimensionality and indexing; all other code is dimension-agnostic.
pub trait Space {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    /// Total cells across all layers — the length of a per-cell array.
    fn len(&self) -> usize;
    fn in_bounds(&self, c: Coord) -> bool;
    /// Flat array index for a cell. Caller ensures `in_bounds`.
    fn index(&self, c: Coord) -> usize;
    /// Same-layer 4-neighbours that are in bounds.
    fn planar_neighbors(&self, c: Coord) -> Vec<Coord>;
}

/// A planar grid with discrete Surface/Underground layers ("2.5D").
#[derive(Clone, Debug)]
pub struct Grid2p5D {
    width: u32,
    height: u32,
}

impl Grid2p5D {
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width > 0 && height > 0, "grid must be non-empty");
        Grid2p5D { width, height }
    }

    fn plane_len(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

impl Space for Grid2p5D {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn len(&self) -> usize {
        LAYER_COUNT * self.plane_len()
    }

    fn in_bounds(&self, c: Coord) -> bool {
        c.x < self.width && c.y < self.height
    }

    fn index(&self, c: Coord) -> usize {
        debug_assert!(self.in_bounds(c));
        (c.layer as usize) * self.plane_len()
            + c.y as usize * self.width as usize
            + c.x as usize
    }

    fn planar_neighbors(&self, c: Coord) -> Vec<Coord> {
        let mut out = Vec::with_capacity(4);
        if c.x > 0 {
            out.push(Coord::new(c.x - 1, c.y, c.layer));
        }
        if c.x + 1 < self.width {
            out.push(Coord::new(c.x + 1, c.y, c.layer));
        }
        if c.y > 0 {
            out.push(Coord::new(c.x, c.y - 1, c.layer));
        }
        if c.y + 1 < self.height {
            out.push(Coord::new(c.x, c.y + 1, c.layer));
        }
        out
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 6 passed`

- [ ] **Step 5: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: Space trait + Grid2p5D"
```

---

### Task 3: `Field` (per-cell scalar storage)

**Files:**
- Create: `src/field.rs`
- Modify: `src/lib.rs` (add `pub mod field;`)

**Interfaces:**
- Consumes: nothing (indexed by `usize` from `Space::index`).
- Produces: `pub struct Field` with `pub fn zeros(len: usize) -> Field`, `pub fn len(&self) -> usize`, `pub fn is_empty(&self) -> bool`, `pub fn get(&self, i: usize) -> f32`, `pub fn set(&mut self, i: usize, v: f32)`, `pub fn add(&mut self, i: usize, dv: f32)`, `pub fn total(&self) -> f32`, `pub fn scale_all(&mut self, factor: f32)`, `pub fn as_slice(&self) -> &[f32]`.

- [ ] **Step 1: Write the failing test**

Add `pub mod field;` to `src/lib.rs` (so it now reads):

```rust
pub mod space;
pub mod field;
```

Create `src/field.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_starts_empty_of_value() {
        let f = Field::zeros(5);
        assert_eq!(f.len(), 5);
        assert_eq!(f.total(), 0.0);
    }

    #[test]
    fn set_get_add_total() {
        let mut f = Field::zeros(3);
        f.set(0, 2.0);
        f.add(0, 0.5);
        f.set(2, 1.0);
        assert_eq!(f.get(0), 2.5);
        assert_eq!(f.get(1), 0.0);
        assert_eq!(f.total(), 3.5);
    }

    #[test]
    fn scale_all_scales_total() {
        let mut f = Field::zeros(2);
        f.set(0, 4.0);
        f.set(1, 6.0);
        f.scale_all(0.5);
        assert_eq!(f.total(), 5.0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find type Field`.

- [ ] **Step 3: Write the minimal implementation**

Insert above the `tests` module in `src/field.rs`:

```rust
//! A scalar field: one `f32` per cell, indexed by a `Space`'s flat index.
//! Stored as a flat `Vec<f32>` so it ports directly to a GPU buffer later.

#[derive(Clone, Debug)]
pub struct Field {
    data: Vec<f32>,
}

impl Field {
    pub fn zeros(len: usize) -> Self {
        Field { data: vec![0.0; len] }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, i: usize) -> f32 {
        self.data[i]
    }

    pub fn set(&mut self, i: usize, v: f32) {
        self.data[i] = v;
    }

    pub fn add(&mut self, i: usize, dv: f32) {
        self.data[i] += dv;
    }

    pub fn total(&self) -> f32 {
        self.data.iter().sum()
    }

    pub fn scale_all(&mut self, factor: f32) {
        for v in &mut self.data {
            *v *= factor;
        }
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 9 passed`

- [ ] **Step 5: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: Field scalar storage"
```

---

### Task 4: `World` + `Params` (construction, sources, access points)

**Files:**
- Create: `src/world.rs`
- Modify: `src/lib.rs` (add `pub mod world;`)

**Interfaces:**
- Consumes: `Space`, `Coord` (Task 2); `Field` (Task 3).
- Produces:
  - `pub struct Params { pub source_rate: f32, pub diffuse_rate: f32, pub layer_exchange: f32, pub decay: f32 }` + `impl Default`.
  - `pub struct World<S: Space> { pub space: S, pub valaar: Field, pub params: Params, /* private sources, access_points */ }` with `pub fn new(space: S, params: Params) -> World<S>`, `pub fn add_source(&mut self, c: Coord)`, `pub fn add_access_point(&mut self, x: u32, y: u32)`, `pub fn sources(&self) -> &[Coord]`, `pub fn access_points(&self) -> &[(u32, u32)]`.

- [ ] **Step 1: Write the failing test**

Add `pub mod world;` to `src/lib.rs` (now reads):

```rust
pub mod space;
pub mod field;
pub mod world;
```

Create `src/world.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    #[test]
    fn new_world_has_zeroed_field_sized_to_space() {
        let space = Grid2p5D::new(4, 4);
        let world = World::new(space, Params::default());
        assert_eq!(world.valaar.len(), world.space.len());
        assert_eq!(world.valaar.total(), 0.0);
        assert!(world.sources().is_empty());
        assert!(world.access_points().is_empty());
    }

    #[test]
    fn sources_and_access_points_record() {
        let space = Grid2p5D::new(4, 4);
        let mut world = World::new(space, Params::default());
        world.add_source(Coord::new(2, 2, Layer::Surface));
        world.add_access_point(2, 2);
        assert_eq!(world.sources(), &[Coord::new(2, 2, Layer::Surface)]);
        assert_eq!(world.access_points(), &[(2, 2)]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find type World` / `Params`.

- [ ] **Step 3: Write the minimal implementation**

Insert above the `tests` module in `src/world.rs`:

```rust
//! The simulated world: a `Space`, the valaar `Field`, and the sources /
//! access points that shape valaar flow.

use crate::field::Field;
use crate::space::{Coord, Space};

/// Tunable rates for valaar dynamics. Every rate is a dial (edge-of-chaos).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Valaar injected at each source cell per step.
    pub source_rate: f32,
    /// Planar diffusion coefficient. Keep `< 0.25` for stability.
    pub diffuse_rate: f32,
    /// Surface<->underground exchange fraction at access points.
    pub layer_exchange: f32,
    /// Fraction of valaar lost per step, in `0.0..=1.0`.
    pub decay: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            source_rate: 1.0,
            diffuse_rate: 0.2,
            layer_exchange: 0.1,
            decay: 0.01,
        }
    }
}

pub struct World<S: Space> {
    pub space: S,
    pub valaar: Field,
    pub params: Params,
    sources: Vec<Coord>,
    access_points: Vec<(u32, u32)>,
}

impl<S: Space> World<S> {
    pub fn new(space: S, params: Params) -> Self {
        let valaar = Field::zeros(space.len());
        World {
            space,
            valaar,
            params,
            sources: Vec::new(),
            access_points: Vec::new(),
        }
    }

    /// Register a valaar source cell (e.g. the Rasconne reservoir).
    pub fn add_source(&mut self, c: Coord) {
        debug_assert!(self.space.in_bounds(c));
        self.sources.push(c);
    }

    /// Register an `(x, y)` column where surface and underground exchange valaar.
    pub fn add_access_point(&mut self, x: u32, y: u32) {
        self.access_points.push((x, y));
    }

    pub fn sources(&self) -> &[Coord] {
        &self.sources
    }

    pub fn access_points(&self) -> &[(u32, u32)] {
        &self.access_points
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 11 passed`

- [ ] **Step 5: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: World + Params"
```

---

### Task 5: Valaar injection + decay

**Files:**
- Create: `src/valaar.rs`
- Modify: `src/lib.rs` (add `pub mod valaar;`)

**Interfaces:**
- Consumes: `Space`, `Coord` (Task 2); `Field` (Task 3).
- Produces: `pub fn inject_sources<S: Space>(space: &S, field: &mut Field, sources: &[Coord], rate: f32)`, `pub fn decay(field: &mut Field, decay: f32)`.

- [ ] **Step 1: Write the failing test**

Add `pub mod valaar;` to `src/lib.rs` (now reads):

```rust
pub mod space;
pub mod field;
pub mod world;
pub mod valaar;
```

Create `src/valaar.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Field;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    #[test]
    fn inject_adds_rate_at_each_source() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        let sources = [Coord::new(1, 1, Layer::Surface), Coord::new(2, 2, Layer::Surface)];
        inject_sources(&space, &mut field, &sources, 3.0);
        assert_eq!(field.get(space.index(Coord::new(1, 1, Layer::Surface))), 3.0);
        assert_eq!(field.get(space.index(Coord::new(2, 2, Layer::Surface))), 3.0);
        assert_eq!(field.total(), 6.0);
    }

    #[test]
    fn decay_scales_total_down() {
        let space = Grid2p5D::new(2, 2);
        let mut field = Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 10.0);
        decay(&mut field, 0.1);
        assert!((field.total() - 9.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find function inject_sources` / `decay`.

- [ ] **Step 3: Write the minimal implementation**

Insert above the `tests` module in `src/valaar.rs`:

```rust
//! Valaar dynamics — pure functions over `(Space, Field)`, composed by
//! `World::step`. Valaar is the world's single energy currency.

use crate::field::Field;
use crate::space::{Coord, Layer, Space};

/// Inject valaar at each source cell (e.g. the Rasconne reservoir).
pub fn inject_sources<S: Space>(space: &S, field: &mut Field, sources: &[Coord], rate: f32) {
    for &c in sources {
        field.add(space.index(c), rate);
    }
}

/// Uniform exponential decay: every cell loses `decay` fraction this step.
pub fn decay(field: &mut Field, decay: f32) {
    field.scale_all(1.0 - decay);
}
```

(The `Coord`, `Layer`, `Space` imports are used by later tasks in this file; keep them.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 13 passed` (an unused-import warning for `Layer` is acceptable; Task 6 uses it).

- [ ] **Step 5: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: valaar injection + decay"
```

---

### Task 6: Planar diffusion (conservative)

**Files:**
- Modify: `src/valaar.rs` (add `diffuse_planar`; extend `tests`)

**Interfaces:**
- Consumes: `Space`, `Coord`, `Layer` (Task 2); `Field` (Task 3).
- Produces: `pub fn diffuse_planar<S: Space>(space: &S, field: &mut Field, rate: f32)`.

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/valaar.rs`:

```rust
    #[test]
    fn diffusion_conserves_total() {
        let space = Grid2p5D::new(8, 8);
        let mut field = Field::zeros(space.len());
        field.set(space.index(Coord::new(4, 4, Layer::Surface)), 100.0);
        let before = field.total();
        for _ in 0..20 {
            diffuse_planar(&space, &mut field, 0.2);
        }
        assert!((field.total() - before).abs() < 1e-3, "total drifted: {}", field.total());
    }

    #[test]
    fn diffusion_spreads_a_spike_to_neighbors() {
        let space = Grid2p5D::new(8, 8);
        let mut field = Field::zeros(space.len());
        let center = Coord::new(4, 4, Layer::Surface);
        field.set(space.index(center), 100.0);
        diffuse_planar(&space, &mut field, 0.2);
        let neighbor = Coord::new(5, 4, Layer::Surface);
        assert!(field.get(space.index(center)) < 100.0, "spike should drop");
        assert!(field.get(space.index(neighbor)) > 0.0, "neighbour should rise");
    }

    #[test]
    fn diffusion_does_not_cross_layers() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        field.set(space.index(Coord::new(2, 2, Layer::Surface)), 50.0);
        diffuse_planar(&space, &mut field, 0.2);
        // underground stays empty: planar diffusion never crosses layers
        assert_eq!(field.get(space.index(Coord::new(2, 2, Layer::Underground))), 0.0);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find function diffuse_planar`.

- [ ] **Step 3: Write the minimal implementation**

Add to `src/valaar.rs` (below `decay`):

```rust
/// Explicit planar diffusion on each layer (discrete Laplacian). Conserves total
/// valaar exactly (no-flux boundaries) because every neighbour exchange is
/// counted symmetrically. Keep `rate < 0.25` for stability.
pub fn diffuse_planar<S: Space>(space: &S, field: &mut Field, rate: f32) {
    let mut delta = vec![0.0f32; field.len()];
    for layer in Layer::ALL {
        for y in 0..space.height() {
            for x in 0..space.width() {
                let c = Coord::new(x, y, layer);
                let i = space.index(c);
                let here = field.get(i);
                for n in space.planar_neighbors(c) {
                    let j = space.index(n);
                    delta[i] += rate * (field.get(j) - here);
                }
            }
        }
    }
    for (i, d) in delta.iter().enumerate() {
        field.add(i, *d);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 16 passed`

- [ ] **Step 5: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: conservative planar diffusion"
```

---

### Task 7: Inter-layer exchange at access points

**Files:**
- Modify: `src/valaar.rs` (add `exchange_layers`; extend `tests`)

**Interfaces:**
- Consumes: `Space`, `Coord`, `Layer` (Task 2); `Field` (Task 3).
- Produces: `pub fn exchange_layers<S: Space>(space: &S, field: &mut Field, access_points: &[(u32, u32)], rate: f32)`.

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/valaar.rs`:

```rust
    #[test]
    fn exchange_moves_valaar_between_layers_and_conserves() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        let surf = space.index(Coord::new(2, 2, Layer::Surface));
        let under = space.index(Coord::new(2, 2, Layer::Underground));
        field.set(under, 10.0); // underground reservoir
        let before = field.total();
        exchange_layers(&space, &mut field, &[(2, 2)], 0.1);
        assert!(field.get(surf) > 0.0, "surface should gain from below");
        assert!(field.get(under) < 10.0, "underground should drop");
        assert!((field.total() - before).abs() < 1e-6, "exchange must conserve");
    }

    #[test]
    fn exchange_only_at_access_points() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        let under = space.index(Coord::new(0, 0, Layer::Underground));
        let surf = space.index(Coord::new(0, 0, Layer::Surface));
        field.set(under, 10.0);
        exchange_layers(&space, &mut field, &[(2, 2)], 0.1); // (0,0) is NOT an access point
        assert_eq!(field.get(surf), 0.0);
        assert_eq!(field.get(under), 10.0);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test`
Expected: **compile error** — `cannot find function exchange_layers`.

- [ ] **Step 3: Write the minimal implementation**

Add to `src/valaar.rs` (below `diffuse_planar`):

```rust
/// Exchange valaar between surface and underground at access points only.
/// Conserves total valaar (flux out of one layer equals flux into the other).
pub fn exchange_layers<S: Space>(
    space: &S,
    field: &mut Field,
    access_points: &[(u32, u32)],
    rate: f32,
) {
    for &(x, y) in access_points {
        let s = space.index(Coord::new(x, y, Layer::Surface));
        let u = space.index(Coord::new(x, y, Layer::Underground));
        let flux = rate * (field.get(u) - field.get(s));
        field.add(s, flux);
        field.add(u, -flux);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test`
Expected: `test result: ok. 18 passed`

- [ ] **Step 5: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: inter-layer valaar exchange"
```

---

### Task 8: `World::step` + headless runner + gradient integration test

**Files:**
- Modify: `src/world.rs` (add `step`)
- Create: `src/bin/sim.rs`
- Create: `tests/gradient.rs`

**Interfaces:**
- Consumes: `World`, `Params` (Task 4); `valaar::{inject_sources, decay, diffuse_planar, exchange_layers}` (Tasks 5–7); `Grid2p5D`, `Coord`, `Layer`, `Space` (Task 2).
- Produces: `World::step(&mut self)`.

- [ ] **Step 1: Write the failing integration test**

Create `tests/gradient.rs`:

```rust
use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::world::{Params, World};

#[test]
fn central_source_builds_a_center_rich_gradient() {
    let (w, h) = (32u32, 32u32);
    let space = Grid2p5D::new(w, h);
    let mut params = Params::default();
    params.decay = 0.0; // isolate gradient formation
    let mut world = World::new(space, params);
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));

    for _ in 0..200 {
        world.step();
    }

    let idx = |x, y| world.space.index(Coord::new(x, y, Layer::Surface));
    let center = world.valaar.get(idx(w / 2, h / 2));
    let edge = world.valaar.get(idx(0, 0));
    assert!(center > edge, "center {center} should exceed edge {edge}");
    assert!(edge >= 0.0, "valaar must stay non-negative");
    assert!(world.valaar.total() > 0.0, "world should hold valaar");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/dev/alife && cargo test --test gradient`
Expected: **compile error** — `no method named step found for struct World`.

- [ ] **Step 3: Write the minimal implementation**

Add a second `impl` block to `src/world.rs` (below the existing one):

```rust
impl<S: Space> World<S> {
    /// Advance one tick: inject sources, diffuse, exchange between layers, decay.
    pub fn step(&mut self) {
        use crate::valaar;
        valaar::inject_sources(&self.space, &mut self.valaar, &self.sources, self.params.source_rate);
        valaar::diffuse_planar(&self.space, &mut self.valaar, self.params.diffuse_rate);
        valaar::exchange_layers(
            &self.space,
            &mut self.valaar,
            &self.access_points,
            self.params.layer_exchange,
        );
        valaar::decay(&mut self.valaar, self.params.decay);
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/dev/alife && cargo test --test gradient`
Expected: `test result: ok. 1 passed`

- [ ] **Step 5: Add the headless runner**

Create `src/bin/sim.rs`:

```rust
//! Headless substrate demo: grow a valaar gradient from a central Rasconne
//! source and print the centre-vs-edge values. No graphics yet.

use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::world::{Params, World};

fn main() {
    let (w, h) = (64u32, 64u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());

    // Rasconne reservoir: a central surface source, linked to the deep reservoir.
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));
    world.add_access_point(w / 2, h / 2);

    let steps = 500;
    for _ in 0..steps {
        world.step();
    }

    let at = |world: &World<Grid2p5D>, x, y, layer| {
        world.valaar.get(world.space.index(Coord::new(x, y, layer)))
    };
    println!(
        "after {steps} steps: total={:.3} center={:.3} edge={:.3} underground_center={:.3}",
        world.valaar.total(),
        at(&world, w / 2, h / 2, Layer::Surface),
        at(&world, 0, 0, Layer::Surface),
        at(&world, w / 2, h / 2, Layer::Underground),
    );
}
```

- [ ] **Step 6: Run the whole suite and the binary**

Run: `cd ~/dev/alife && cargo test`
Expected: all green — the library unit tests report `test result: ok. 18 passed` and the `gradient` integration test reports `test result: ok. 1 passed`.

Run: `cd ~/dev/alife && cargo run --bin sim`
Expected: a line like `after 500 steps: total=... center=... edge=... underground_center=...` where `center > edge` and `underground_center > 0` (valaar reached the deep reservoir through the access point).

- [ ] **Step 7: Checkpoint** (local only)

```bash
# optional: git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: World::step + headless sim + gradient test"
```

---

## What this delivers

A runnable, fully-tested **world substrate**: a 2.5D space behind a dimension-agnostic `Space` trait, and a valaar field that flows from Rasconne, diffuses, seeps to the underground at access points, and decays — yielding the emergent centre-rich → Dusk-poor gradient that the whole biosphere will later sit on. The 2D interface and the documented 3D seam are in place from the first task.

**Next plans (not in scope here):** trait-vector organisms; the ecology/energy loop; heat + water fields and the 6-arh season cycle (Vraze = crystallisation); mutation-as-a-field; wgpu acceleration + rendering; the web viewer; the natural-history/species layer.
