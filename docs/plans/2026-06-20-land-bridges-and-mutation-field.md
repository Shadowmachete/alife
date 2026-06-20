# Plan 5 — Periodic Land Bridges + Mutation-as-a-Field Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two deterministic *disturbance* systems to the dynamic world: (1) **periodic land bridges** that rise across narrow ocean straits during the Vraze quake season, giving intermittent gene flow between continents, and (2) **mutation-as-a-field**, where an organism's mutation magnitude at birth scales with the local valaar density (Dusk conservative → Rasconne core fast) and spikes during Rasgun.

**Architecture:** Land bridges are a new `src/bridges.rs` lib module: `find_bridge_sites` carves ribbon corridors (3–6 wide) across narrow ocean gaps between *different* continents once at load; a `Bridges` controller (with its **own RNG**, so the ecology stream is untouched and old seeds reproduce) rolls — per Vraze, per site — whether/when/how-long each opens, and each tick reports the cells whose passability changed. `Sim` owns an `Option<Bridges>`, flips those cells in `World`'s passability mask via a new `World::set_cell_passable`, and drowns (`ecology::drown`) any organism on a just-sunk cell. Mutation-as-a-field is a pure `ecology::mutation_rate(eco, local_valaar, season)` consumed by `reproduce`, which gains `space`/`valaar`/`season` parameters; the RNG draw count is unchanged so determinism holds.

**Tech Stack:** Rust (edition 2021). No new dependencies. Bridges are a feature of the terrain/`mapview` path; the headless `life`/`sim` binaries get no bridges. Mutation-as-a-field applies everywhere (a valaar field always exists).

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** brainstorm of 2026-06-20 (this session) over `~/dev/ideas/world-design.md` §F (Vraze land bridges; mutation-as-valaar-field) and the plan-5 bullet in `~/dev/ideas/artificial-life-sim.md`.

**Builds on (shipped — exact interfaces consumed):**

- `space::{Coord, Layer, Space, Grid2p5D}` — `Coord{x,y,layer}`, `Space::{width,height,len,in_bounds,index,planar_neighbors}`; surface plane index of `(x,y)` is `y*sw + x` (layer 0).
- `terrain::CellType` — `{ Ocean, Land, Valaar, Rock, Mountain }`, `==` via `PartialEq`.
- `field::Field` — `zeros(len)`, `get(i)`, `set(i, v)`.
- `rng::Rng` — `new(seed: u64)`, `next_unit() -> f32` in `[0,1)`, `next_range(lo, hi)`.
- `season::{Season, Calendar, CRAWS_PER_ARH}` — `Season::Vraze.index() == 3`, `Season::Rasgun`; `Calendar::{craw(), year(), season()}`; Vraze spans craws `[3*117, 4*117) = [351, 468)`.
- `world::World<S>` — pub `space`, `valaar`; private `passability: Option<Vec<bool>>` with `set_passability(Vec<bool>)` / `passability() -> Option<&[bool]>`.
- `mapsim::{label_continents, world_from_materials, downscale}` — `label_continents(mats, sw, sh) -> (Vec<Option<u32>>, u32)` (per-cell continent label, `None` for Ocean).
- `organism::TraitOrganism` — `new(genome, pos, energy)`, pub `genome`/`pos`/`energy`; `genome.mutate(rng, rate)`.
- `population::Population` — `organisms()`, `organisms_mut()`, `spawn`, `len`, `retain(|&o| ...)`.
- `ecology::reproduce(pop, eco, rng)` — current signature (being changed in Task 6).
- `sim::Sim<S>` — pub `world`/`pop`/`eco`/`rng`/`calendar`/`heat`/`water`; `step()`; `with_climate`/`new`; tick order `absorb → move_organisms → predate → environmental_stress → metabolize → cull_and_recycle → reproduce`.
- `bin/mapview.rs::load_scene` — builds `sim_mats`, `(continents, n_continents) = label_continents(...)`, `world = world_from_materials(...)`, `sim = Sim::new(world, EcoParams::default(), 0xA11FE)`, then `WARM_STEPS` of `sim.world.step()`.

## Global Constraints

- **Language:** Rust, edition 2021. No new dependencies.
- **Formatting / lint gate (repo convention):** repo is **not** rustfmt-clean — **do NOT run `cargo fmt`**. Hand-format to match surrounding code (4-space indent, `//!`/`///` docs). Gate every task on `cargo test` and `cargo clippy --all-targets`.
- **Determinism:** no wall-clock. Bridges use a **dedicated RNG** seeded separately, so the main ecology RNG stream (movement/reproduction) is byte-for-byte unchanged for seeds that don't use bridges. `reproduce` keeps drawing the RNG exactly the same number of times per child (only the perturbation *magnitude* changes). All scans are fixed-order (row-major; horizontal then vertical).
- **Layering:** logic lives in lib modules with unit tests; the `eframe` `mapview` binary is wiring only (verified by build + clippy + a manual run).
- **Surface-plane indices:** bridge cells and the masks they flip are surface cells `0..sw*sh` (`Space::index` of a `Layer::Surface` coord). Underground organisms never collide with them.
- **Version control:** work on `main` (per user). Commit per task, plain message, **no `Co-Authored-By` trailer**. `git add` only the files each task names (the working tree also carries unrelated user edits in `params.rs`/`climate.rs`/`terrain.rs`/`.gitignore`).

---

## Design assumptions (flagged for veto)

- **A1 — Bridge sites are computed at load, seeded-random subset, 3–6 wide.** Narrow straight ocean runs (≤ `max_gap`) flanked by two *different* continents become ribbon corridors widened perpendicular into Ocean.
- **A2 — Bridges open only during Vraze.** Per Vraze, each site opens with probability `open_fraction`, at a random craw-offset into the arh, for a random duration — so the open set and timing vary year to year. Closed = revert to ocean.
- **A3 — Drowning.** When a bridge cell sinks, any organism on it is culled that tick.
- **A4 — Bridges are terrain-path only.** Attached in `mapview::load_scene`; headless bins have `None`.
- **A5 — Mutation-as-a-field is global and always on.** `rate = mutation_rate × lerp(floor_mult, ceil_mult, local_valaar/ref) × (rasgun ? rasgun_mult : 1)`. No on/off flag (per user: "fine").

---

## File Structure

- `src/bridges.rs` — **new**: `BridgeConfig`, `BridgeSite`, `find_bridge_sites`, `Bridges`, `BridgeUpdate`.
- `src/lib.rs` — **modified**: register `pub mod bridges;`.
- `src/world.rs` — **modified**: `set_cell_passable` + tests.
- `src/ecology.rs` — **modified**: `drown`; `mutation_rate`; `reproduce` signature; tests.
- `src/params.rs` — **modified**: four mutation-field dials + a test.
- `src/sim.rs` — **modified**: `bridges` field + `set_bridges` + tick integration + new `reproduce` call.
- `src/bin/mapview.rs` — **modified**: compute + attach bridges in `load_scene`.
- `README.md` — **modified**: note land bridges + mutation field.

---

### Task 1: `bridges` module — sites

**Files:**
- Create: `src/bridges.rs`
- Modify: `src/lib.rs` (register module)

**Interfaces:**
- Produces: `BridgeConfig` (Copy, Default); `BridgeSite { cells: Vec<usize> }`; `find_bridge_sites(mats: &[CellType], sw: u32, sh: u32, labels: &[Option<u32>], rng: &mut Rng, cfg: &BridgeConfig) -> Vec<BridgeSite>`.

- [ ] **Step 1: Register the module**

In `src/lib.rs`, add after `pub mod mapsim;`:

```rust
pub mod bridges;
```

- [ ] **Step 2: Write the module with the failing tests**

Create `src/bridges.rs`:

```rust
//! Periodic land bridges: a Vraze-season disturbance that raises temporary land
//! across narrow ocean straits between continents, giving intermittent gene flow
//! (allopatric speciation with reconnection). Sites are found once at load; the
//! `Bridges` controller (Task 2) schedules which open, when, and for how long.

use crate::rng::Rng;
use crate::terrain::CellType;

/// Tunable dials for bridge generation and scheduling.
#[derive(Clone, Copy, Debug)]
pub struct BridgeConfig {
    /// Longest ocean span (cells) that can become a bridge.
    pub max_gap: u32,
    /// Ribbon width range (cells), inclusive.
    pub min_width: u32,
    pub max_width: u32,
    /// Fraction of candidate straits kept as sites at load.
    pub site_fraction: f32,
    /// Fraction of sites that open in any given Vraze.
    pub open_fraction: f32,
    /// A bridge stays open this many craws (range, clamped into the Vraze arh).
    pub min_duration: u32,
    pub max_duration: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        BridgeConfig {
            max_gap: 8,
            min_width: 3,
            max_width: 6,
            site_fraction: 0.5,
            open_fraction: 0.5,
            min_duration: 20,
            max_duration: 80,
        }
    }
}

/// One bridge corridor: the surface-plane cell indices (`y*sw + x`) of its ocean
/// ribbon. Flipping these passable opens the bridge; reverting closes it.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeSite {
    pub cells: Vec<usize>,
}

/// Find candidate bridge corridors and keep a seeded-random subset. A candidate
/// is a straight run of Ocean cells (≤ `cfg.max_gap` long) flanked at both ends
/// by land of two *different* continents; it is widened perpendicular into a
/// ribbon `cfg.min_width..=cfg.max_width` cells across (Ocean cells only). Cells
/// are claimed so sites never overlap. Deterministic over `(mats, labels, rng,
/// cfg)` with a fixed horizontal-then-vertical, row-major scan.
pub fn find_bridge_sites(
    mats: &[CellType],
    sw: u32,
    sh: u32,
    labels: &[Option<u32>],
    rng: &mut Rng,
    cfg: &BridgeConfig,
) -> Vec<BridgeSite> {
    let idx = |x: u32, y: u32| (y * sw + x) as usize;
    let is_ocean = |x: u32, y: u32| mats[idx(x, y)] == CellType::Ocean;

    // (run_cells, widen_along_y): a horizontal strait widens along y; vertical along x.
    let mut candidates: Vec<(Vec<usize>, bool)> = Vec::new();

    // Horizontal straits: ocean runs along x at a fixed row, land on left/right.
    for y in 0..sh {
        let mut x = 0u32;
        while x < sw {
            if !is_ocean(x, y) {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < sw && is_ocean(x, y) {
                x += 1;
            }
            let x1 = x - 1;
            let len = x1 - x0 + 1;
            if x0 == 0 || x1 + 1 >= sw || len > cfg.max_gap {
                continue;
            }
            if let (Some(a), Some(b)) = (labels[idx(x0 - 1, y)], labels[idx(x1 + 1, y)]) {
                if a != b {
                    candidates.push(((x0..=x1).map(|xx| idx(xx, y)).collect(), true));
                }
            }
        }
    }

    // Vertical straits: ocean runs along y at a fixed column, land on top/bottom.
    for x in 0..sw {
        let mut y = 0u32;
        while y < sh {
            if !is_ocean(x, y) {
                y += 1;
                continue;
            }
            let y0 = y;
            while y < sh && is_ocean(x, y) {
                y += 1;
            }
            let y1 = y - 1;
            let len = y1 - y0 + 1;
            if y0 == 0 || y1 + 1 >= sh || len > cfg.max_gap {
                continue;
            }
            if let (Some(a), Some(b)) = (labels[idx(x, y0 - 1)], labels[idx(x, y1 + 1)]) {
                if a != b {
                    candidates.push(((y0..=y1).map(|yy| idx(x, yy)).collect(), false));
                }
            }
        }
    }

    let mut claimed = vec![false; (sw * sh) as usize];
    let mut sites = Vec::new();
    for (run, widen_y) in candidates {
        if rng.next_unit() >= cfg.site_fraction {
            continue; // keep a random subset
        }
        if run.iter().any(|&c| claimed[c]) {
            continue; // overlaps an already-carved site
        }
        let span = (cfg.max_width.saturating_sub(cfg.min_width) + 1) as f32;
        let width = (cfg.min_width + (rng.next_unit() * span) as u32).clamp(cfg.min_width, cfg.max_width);
        let mut cells = Vec::new();
        for &c in &run {
            let cx = (c as u32) % sw;
            let cy = (c as u32) / sw;
            for w in 0..width {
                let (nx, ny) = if widen_y { (cx, cy + w) } else { (cx + w, cy) };
                if nx >= sw || ny >= sh {
                    break;
                }
                let ni = idx(nx, ny);
                if mats[ni] == CellType::Ocean && !claimed[ni] {
                    claimed[ni] = true;
                    cells.push(ni);
                }
            }
        }
        if !cells.is_empty() {
            cells.sort_unstable();
            sites.push(BridgeSite { cells });
        }
    }
    sites
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapsim::label_continents;

    // 'O' -> Ocean, else Land. Returns (sw, sh, mats).
    fn grid(rows: &[&str]) -> (u32, u32, Vec<CellType>) {
        let sh = rows.len() as u32;
        let sw = rows[0].len() as u32;
        let mut mats = Vec::with_capacity((sw * sh) as usize);
        for r in rows {
            for ch in r.chars() {
                mats.push(if ch == 'O' { CellType::Ocean } else { CellType::Land });
            }
        }
        (sw, sh, mats)
    }

    fn cfg_keep_all(min_width: u32, max_width: u32, max_gap: u32) -> BridgeConfig {
        BridgeConfig { max_gap, min_width, max_width, site_fraction: 1.0, ..BridgeConfig::default() }
    }

    #[test]
    fn finds_a_strait_between_two_continents() {
        let (sw, sh, m) = grid(&["LLOOLL"]); // cont0 | ocean | cont1
        let (labels, n) = label_continents(&m, sw, sh);
        assert_eq!(n, 2);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(1, 1, 4));
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].cells, vec![2, 3]); // the two ocean cells
    }

    #[test]
    fn ignores_gaps_wider_than_max_gap() {
        let (sw, sh, m) = grid(&["LLOOOOOOLL"]); // 6-wide ocean
        let (labels, _) = label_continents(&m, sw, sh);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(1, 1, 4));
        assert!(sites.is_empty(), "gap of 6 > max_gap 4");
    }

    #[test]
    fn ignores_ocean_within_one_continent() {
        let (sw, sh, m) = grid(&["LOL", "LLL"]); // the L's all connect around the lake
        let (labels, n) = label_continents(&m, sw, sh);
        assert_eq!(n, 1);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(1, 1, 4));
        assert!(sites.is_empty(), "same continent on both sides -> not a strait");
    }

    #[test]
    fn widens_into_a_ribbon_and_claims_cells() {
        let (sw, sh, m) = grid(&["LLOOLL", "LLOOLL", "LLOOLL"]);
        let (labels, _) = label_continents(&m, sw, sh);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(3, 3, 4));
        assert_eq!(sites.len(), 1, "rows after the first reuse claimed cells");
        assert_eq!(sites[0].cells.len(), 6, "3 rows x 2 ocean columns");
    }

    #[test]
    fn is_seed_deterministic() {
        let (sw, sh, m) = grid(&["LLOOLL", "LLOOLL"]);
        let (labels, _) = label_continents(&m, sw, sh);
        let a = find_bridge_sites(&m, sw, sh, &labels, &mut Rng::new(9), &BridgeConfig::default());
        let b = find_bridge_sites(&m, sw, sh, &labels, &mut Rng::new(9), &BridgeConfig::default());
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 3: Run to verify it compiles and the tests pass**

Run: `cd ~/dev/alife && cargo test --lib bridges`
Expected: PASS (5 tests). (The tests *are* the failing-first gate: before the module existed they could not compile.)

- [ ] **Step 4: Lint**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/bridges.rs
git commit -m "feat: bridge-site detection (narrow inter-continent straits)"
```

---

### Task 2: `Bridges` controller (scheduling + per-tick update)

**Files:**
- Modify: `src/bridges.rs` (add `Bridges`, `BridgeUpdate` + tests)

**Interfaces:**
- Consumes: `BridgeSite`, `BridgeConfig` (Task 1), `season::{Calendar, Season, CRAWS_PER_ARH}`.
- Produces: `BridgeUpdate { opened: Vec<usize>, closed: Vec<usize> }`; `Bridges::new(sites, cfg, seed) -> Bridges`; `Bridges::site_count(&self) -> usize`; `Bridges::update(&mut self, calendar: &Calendar) -> BridgeUpdate`.

- [ ] **Step 1: Add the import**

In `src/bridges.rs`, extend the imports at the top:

```rust
use crate::rng::Rng;
use crate::season::{Calendar, Season, CRAWS_PER_ARH};
use crate::terrain::CellType;
```

- [ ] **Step 2: Write the failing tests**

Add to the `tests` module in `src/bridges.rs`:

```rust
    fn cal_at(target_craw: u32) -> Calendar {
        let mut c = Calendar::new();
        while c.craw() < target_craw {
            c.advance();
        }
        c
    }

    #[test]
    fn closed_outside_vraze() {
        let cfg = BridgeConfig { open_fraction: 1.0, ..BridgeConfig::default() };
        let mut b = Bridges::new(vec![BridgeSite { cells: vec![5, 6] }], cfg, 7);
        let upd = b.update(&cal_at(150)); // Goscon
        assert!(upd.opened.is_empty() && upd.closed.is_empty());
    }

    #[test]
    fn opens_then_closes_within_vraze() {
        let cfg = BridgeConfig { open_fraction: 1.0, min_duration: 10, max_duration: 10, ..BridgeConfig::default() };
        let mut b = Bridges::new(vec![BridgeSite { cells: vec![5, 6] }], cfg, 7);
        let mut opened_at = None;
        let mut closed_at = None;
        let mut c = cal_at(3 * CRAWS_PER_ARH); // start of Vraze
        for _ in 0..CRAWS_PER_ARH {
            let upd = b.update(&c);
            if !upd.opened.is_empty() {
                assert_eq!(upd.opened, vec![5, 6]);
                opened_at = Some(c.craw());
            }
            if !upd.closed.is_empty() {
                assert_eq!(upd.closed, vec![5, 6]);
                closed_at = Some(c.craw());
            }
            c.advance();
        }
        let (o, cl) = (opened_at.expect("opened in Vraze"), closed_at.expect("closed in Vraze"));
        assert!(cl > o, "closes after it opens");
    }

    #[test]
    fn schedule_is_seed_deterministic() {
        let cfg = BridgeConfig { open_fraction: 0.5, ..BridgeConfig::default() };
        let sites = vec![BridgeSite { cells: vec![1] }, BridgeSite { cells: vec![2] }, BridgeSite { cells: vec![3] }];
        let run = |seed| {
            let mut b = Bridges::new(sites.clone(), cfg, seed);
            let mut log = Vec::new();
            let mut c = cal_at(3 * CRAWS_PER_ARH);
            for _ in 0..CRAWS_PER_ARH {
                let u = b.update(&c);
                log.push((u.opened, u.closed));
                c.advance();
            }
            log
        };
        assert_eq!(run(123), run(123));
    }
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib bridges`
Expected: FAIL — `cannot find type Bridges` / `cannot find function ... new`.

- [ ] **Step 4: Implement the controller**

In `src/bridges.rs`, add after `find_bridge_sites` (before the `tests` module):

```rust
/// Cells whose passability changed this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeUpdate {
    pub opened: Vec<usize>,
    pub closed: Vec<usize>,
}

/// Runtime controller: owns the sites, their open/closed state, the current
/// Vraze's per-site schedule, and a **dedicated RNG** so the main ecology RNG
/// stream is untouched (old seeds reproduce identically).
pub struct Bridges {
    sites: Vec<BridgeSite>,
    cfg: BridgeConfig,
    open: Vec<bool>,
    /// `(open_craw, close_craw)` within the year for the scheduled Vraze, per
    /// site; `None` = this site does not open this Vraze.
    window: Vec<Option<(u32, u32)>>,
    scheduled_year: Option<u32>,
    rng: Rng,
}

impl Bridges {
    pub fn new(sites: Vec<BridgeSite>, cfg: BridgeConfig, seed: u64) -> Self {
        let n = sites.len();
        Bridges { sites, cfg, open: vec![false; n], window: vec![None; n], scheduled_year: None, rng: Rng::new(seed) }
    }

    pub fn site_count(&self) -> usize {
        self.sites.len()
    }

    /// Roll a fresh schedule for the current Vraze: each site opens with prob
    /// `open_fraction`, at a random offset into the Vraze arh, for a random
    /// duration (clamped inside the arh).
    fn schedule(&mut self) {
        let cfg = self.cfg;
        let vraze_start = Season::Vraze.index() as u32 * CRAWS_PER_ARH;
        let vraze_end = vraze_start + CRAWS_PER_ARH;
        for i in 0..self.window.len() {
            if self.rng.next_unit() < cfg.open_fraction {
                let span = (cfg.max_duration.saturating_sub(cfg.min_duration) + 1) as f32;
                let dur = (cfg.min_duration + (self.rng.next_unit() * span) as u32).clamp(1, CRAWS_PER_ARH);
                let latest_start = CRAWS_PER_ARH.saturating_sub(dur);
                let off = ((self.rng.next_unit() * (latest_start + 1) as f32) as u32).min(latest_start);
                let open_craw = vraze_start + off;
                let close_craw = (open_craw + dur).min(vraze_end);
                self.window[i] = Some((open_craw, close_craw));
            } else {
                self.window[i] = None;
            }
        }
    }

    /// Advance one tick. Rolls a new schedule at the first Vraze tick of each
    /// year; flips each site open/closed by the current craw; returns the cells
    /// whose passability changed this tick.
    pub fn update(&mut self, calendar: &Calendar) -> BridgeUpdate {
        let mut upd = BridgeUpdate::default();
        let in_vraze = calendar.season() == Season::Vraze;
        if in_vraze && self.scheduled_year != Some(calendar.year()) {
            self.scheduled_year = Some(calendar.year());
            self.schedule();
        }
        let craw = calendar.craw();
        for i in 0..self.sites.len() {
            let want_open = in_vraze
                && matches!(self.window[i], Some((o, c)) if craw >= o && craw < c);
            if want_open && !self.open[i] {
                self.open[i] = true;
                upd.opened.extend_from_slice(&self.sites[i].cells);
            } else if !want_open && self.open[i] {
                self.open[i] = false;
                upd.closed.extend_from_slice(&self.sites[i].cells);
            }
        }
        upd
    }
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib bridges && cargo clippy --all-targets`
Expected: PASS (8 tests), clippy clean.

- [ ] **Step 6: Commit**

```bash
git add src/bridges.rs
git commit -m "feat: Bridges controller — per-Vraze open/close scheduling"
```

---

### Task 3: `World::set_cell_passable`

**Files:**
- Modify: `src/world.rs` (method + tests)

**Interfaces:**
- Produces: `World::set_cell_passable(&mut self, index: usize, passable: bool)` — flips one cell in the passability mask; no-op if no mask is installed.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/world.rs`:

```rust
    #[test]
    fn set_cell_passable_flips_one_cell() {
        let space = Grid2p5D::new(2, 2);
        let mut world = World::new(space, Params::default());
        world.set_passability(vec![false; world.space.len()]);
        world.set_cell_passable(2, true);
        assert!(world.passability().unwrap()[2]);
        world.set_cell_passable(2, false);
        assert!(!world.passability().unwrap()[2]);
    }

    #[test]
    fn set_cell_passable_is_noop_without_a_mask() {
        let space = Grid2p5D::new(2, 2);
        let mut world = World::new(space, Params::default());
        world.set_cell_passable(0, true); // must not panic
        assert!(world.passability().is_none());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib world`
Expected: FAIL — `no method named set_cell_passable`.

- [ ] **Step 3: Implement the method**

In `src/world.rs`, add inside the `impl<S: Space> World<S>` block that holds `set_passability` (after `swimmable`):

```rust
    /// Flip a single cell's passability (used by dynamic terrain such as land
    /// bridges). No-op if no passability mask is installed.
    pub fn set_cell_passable(&mut self, index: usize, passable: bool) {
        if let Some(mask) = self.passability.as_mut() {
            mask[index] = passable;
        }
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib world && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/world.rs
git commit -m "feat: World::set_cell_passable for dynamic terrain"
```

---

### Task 4: `ecology::drown`

**Files:**
- Modify: `src/ecology.rs` (function + test)

**Interfaces:**
- Produces: `drown<S: Space>(space: &S, pop: &mut Population, drowned: &[usize])` — removes organisms whose `space.index(pos)` is in `drowned` (surface-plane cell indices).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn drown_removes_organisms_on_sunk_cells() {
        let space = Grid2p5D::new(3, 1);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(genome(0.0, 1.0), Coord::new(0, 0, Layer::Surface), 1.0));
        pop.spawn(TraitOrganism::new(genome(0.0, 1.0), Coord::new(1, 0, Layer::Surface), 1.0));
        let sunk = vec![space.index(Coord::new(1, 0, Layer::Surface))];
        drown(&space, &mut pop, &sunk);
        assert_eq!(pop.len(), 1);
        assert_eq!(pop.organisms()[0].pos, Coord::new(0, 0, Layer::Surface));
    }
```

(`genome(diet, eff)` is an existing helper in the `ecology` tests module.)

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: FAIL — `cannot find function drown`.

- [ ] **Step 3: Implement `drown`**

In `src/ecology.rs`, add next to the other tick functions (e.g. after `cull_and_recycle`):

```rust
/// Remove organisms standing on any of `drowned` (surface-plane cell indices) —
/// e.g. when a land bridge sinks back to ocean beneath them.
pub fn drown<S: Space>(space: &S, pop: &mut Population, drowned: &[usize]) {
    if drowned.is_empty() {
        return;
    }
    pop.retain(|o| !drowned.contains(&space.index(o.pos)));
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib ecology && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/ecology.rs
git commit -m "feat: ecology::drown — cull organisms on sunk bridge cells"
```

---

### Task 5: Mutation-as-a-field — `EcoParams` dials + `mutation_rate`

**Files:**
- Modify: `src/params.rs` (four fields + a test)
- Modify: `src/ecology.rs` (`mutation_rate` + tests)

**Interfaces:**
- Produces: `EcoParams.{mutation_floor_mult, mutation_ceil_mult, mutation_ref, rasgun_mutation_mult}: f32`; `ecology::mutation_rate(eco: &EcoParams, local_valaar: f32, season: Season) -> f32`.

- [ ] **Step 1: Write the failing params test**

Add to the `tests` module in `src/params.rs`:

```rust
    #[test]
    fn mutation_field_defaults_present() {
        let eco = EcoParams::default();
        assert!(eco.mutation_floor_mult < eco.mutation_ceil_mult);
        assert!(eco.mutation_ref > 0.0);
        assert!(eco.rasgun_mutation_mult > 1.0);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib params`
Expected: FAIL — `no field mutation_floor_mult`.

- [ ] **Step 3: Add the fields**

In `src/params.rs`, add to `struct EcoParams` after `valaar_drain`:

```rust
    /// Energy a tunneller spends per Valaar cell crossed when it swims through.
    pub valaar_drain: f32,
    /// Mutation magnitude multiplier where local valaar is ~0 (the Dusk).
    pub mutation_floor_mult: f32,
    /// Mutation magnitude multiplier where local valaar >= `mutation_ref` (core).
    pub mutation_ceil_mult: f32,
    /// Local valaar at/above which mutation reaches `mutation_ceil_mult`.
    pub mutation_ref: f32,
    /// Extra mutation multiplier during the Rasgun surge.
    pub rasgun_mutation_mult: f32,
}
```

In the `Default` impl, after `valaar_drain: 0.15,` (keep the user's value):

```rust
            valaar_drain: 0.15,
            mutation_floor_mult: 0.25,
            mutation_ceil_mult: 2.0,
            mutation_ref: 4.0,
            rasgun_mutation_mult: 2.0,
        }
```

- [ ] **Step 4: Write the failing `mutation_rate` tests**

In `src/ecology.rs`, add `use crate::season::Season;` to the imports at the top of the file. Then add to the `tests` module:

```rust
    #[test]
    fn mutation_rate_floors_in_the_dusk_and_peaks_in_the_core() {
        let eco = EcoParams::default();
        let dusk = mutation_rate(&eco, 0.0, Season::Goscon);
        let core = mutation_rate(&eco, eco.mutation_ref, Season::Goscon);
        assert!((dusk - eco.mutation_rate * eco.mutation_floor_mult).abs() < 1e-6);
        assert!((core - eco.mutation_rate * eco.mutation_ceil_mult).abs() < 1e-6);
        assert!(core > dusk);
    }

    #[test]
    fn rasgun_amplifies_mutation() {
        let eco = EcoParams::default();
        let normal = mutation_rate(&eco, eco.mutation_ref, Season::Goscon);
        let rasgun = mutation_rate(&eco, eco.mutation_ref, Season::Rasgun);
        assert!((rasgun - normal * eco.rasgun_mutation_mult).abs() < 1e-5);
    }
```

- [ ] **Step 5: Run to verify both fail**

Run: `cd ~/dev/alife && cargo test --lib params && cargo test --lib ecology`
Expected: params PASS now (Step 3 done); ecology FAIL — `cannot find function mutation_rate`.

- [ ] **Step 6: Implement `mutation_rate`**

In `src/ecology.rs`, add near `reproduce`:

```rust
/// Per-organism mutation magnitude at birth: the base `mutation_rate` scaled by
/// local valaar (Dusk -> `mutation_floor_mult`, Rasconne core -> `mutation_ceil_mult`,
/// linear between, normalised against `mutation_ref`) and multiplied by
/// `rasgun_mutation_mult` during Rasgun.
pub fn mutation_rate(eco: &EcoParams, local_valaar: f32, season: Season) -> f32 {
    let t = (local_valaar / eco.mutation_ref).clamp(0.0, 1.0);
    let valaar_mult = eco.mutation_floor_mult + (eco.mutation_ceil_mult - eco.mutation_floor_mult) * t;
    let season_mult = if season == Season::Rasgun { eco.rasgun_mutation_mult } else { 1.0 };
    eco.mutation_rate * valaar_mult * season_mult
}
```

- [ ] **Step 7: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 8: Commit**

```bash
git add src/params.rs src/ecology.rs
git commit -m "feat: mutation-as-a-field rate (valaar-scaled, Rasgun spike)"
```

---

### Task 6: `reproduce` consumes the mutation field

**Files:**
- Modify: `src/ecology.rs` (`reproduce` signature + body; update its two existing tests; add a divergence test)
- Modify: `src/sim.rs` (update the `reproduce` call)

**Interfaces:**
- Consumes: `mutation_rate` (Task 5), `season::Season`.
- Produces: `reproduce<S: Space>(space: &S, valaar: &Field, pop: &mut Population, eco: &EcoParams, rng: &mut Rng, season: Season)`.

- [ ] **Step 1: Update the two existing reproduce tests + add a divergence test**

In `src/ecology.rs`, change the calls in `well_fed_organism_spawns_one_child` and `starving_organism_does_not_reproduce` from `reproduce(&mut pop, &eco, &mut rng)` to:

```rust
        let space = Grid2p5D::new(2, 2);
        let valaar = crate::field::Field::zeros(space.len());
        reproduce(&space, &valaar, &mut pop, &eco, &mut rng, Season::Goscon);
```

(Insert the `space`/`valaar` lets just before each `reproduce(...)` call; `Grid2p5D`, `Coord`, `Layer`, `Season` are already in scope via the tests' `use` lines plus the Task-5 import.)

Then add a new test:

```rust
    #[test]
    fn reproduce_mutates_more_where_valaar_is_high() {
        let space = Grid2p5D::new(2, 1);
        let eco = EcoParams::default();
        let dusk = Coord::new(0, 0, Layer::Surface); // valaar 0
        let core = Coord::new(1, 0, Layer::Surface);
        let mut valaar = crate::field::Field::zeros(space.len());
        valaar.set(space.index(core), eco.mutation_ref * 4.0); // saturate to ceil
        // repro_threshold 0 so any energy reproduces.
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.5, 0.5]);
        let child_genome = |pos| {
            let mut pop = Population::new();
            pop.spawn(TraitOrganism::new(g, pos, 5.0));
            let mut rng = Rng::new(7);
            reproduce(&space, &valaar, &mut pop, &eco, &mut rng, Season::Goscon);
            pop.organisms()[1].genome
        };
        let delta = |gc: Genome| -> f32 {
            gc.to_array().iter().zip(g.to_array()).map(|(a, b)| (a - b).abs()).sum()
        };
        assert!(
            delta(child_genome(core)) > delta(child_genome(dusk)),
            "the core mutates further than the dusk on the same rng draws"
        );
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: FAIL — arity mismatch on `reproduce` (compile error).

- [ ] **Step 3: Update `reproduce`**

In `src/ecology.rs`, replace the `reproduce` function:

```rust
/// Asexual reproduction: any organism at or above its energy threshold spawns
/// one child in its own cell, taking `repro_cost_fraction` of the parent's
/// energy and a mutated copy of its genome. The mutation magnitude is the
/// **mutation field** at the parent's cell (`mutation_rate`). Children are
/// collected first, then appended, so iteration order (determinism) is stable.
pub fn reproduce<S: Space>(
    space: &S,
    valaar: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
    season: Season,
) {
    let mut children: Vec<TraitOrganism> = Vec::new();
    for o in pop.organisms_mut() {
        let threshold = o.genome.repro_threshold * o.max_energy(eco);
        if o.energy >= threshold && o.energy > 0.0 {
            let child_energy = o.energy * eco.repro_cost_fraction;
            o.energy -= child_energy;
            let rate = mutation_rate(eco, valaar.get(space.index(o.pos)), season);
            let child_genome = o.genome.mutate(rng, rate);
            children.push(TraitOrganism::new(child_genome, o.pos, child_energy));
        }
    }
    for c in children {
        pop.spawn(c);
    }
}
```

- [ ] **Step 4: Update the `Sim::step` call**

In `src/sim.rs`, change the `reproduce` call (last line of `step`) to:

```rust
        ecology::reproduce(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
            season,
        );
```

(`season` is already bound near the top of `step` as `let season = self.calendar.season();`.)

- [ ] **Step 5: Run the full suite**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS, clippy clean. (The headless integration tests in `tests/ecology.rs` still pass — mutation-field is deterministic and only changes perturbation magnitude.)

- [ ] **Step 6: Commit**

```bash
git add src/ecology.rs src/sim.rs
git commit -m "feat: reproduce draws mutation rate from the valaar field"
```

---

### Task 7: Wire bridges into `Sim` + `mapview`

**Files:**
- Modify: `src/sim.rs` (`bridges` field, `set_bridges`, tick integration)
- Modify: `src/bin/mapview.rs` (compute + attach bridges in `load_scene`)

**Interfaces:**
- Consumes: `bridges::{Bridges, BridgeConfig, find_bridge_sites, BridgeUpdate}` (Tasks 1–2), `World::set_cell_passable` (Task 3), `ecology::drown` (Task 4).
- Produces: `Sim::set_bridges(&mut self, bridges: Bridges)`; bridges driven each tick inside `Sim::step`.

> No new unit test (composition of tested parts); verified by full suite + clippy + a manual run.

- [ ] **Step 1: Add the field + setter to `Sim`**

In `src/sim.rs`, add the import:

```rust
use crate::bridges::Bridges;
```

Add a field to `struct Sim<S: Space>` (after `climate`):

```rust
    pub climate: Climate,
    /// Optional dynamic land bridges (terrain path only; `None` headless).
    bridges: Option<Bridges>,
```

In **both** constructors set it — `with_climate` builds the struct literal, so add `bridges: None,` there (it's the only place the literal is built; `new` delegates to `with_climate`). Add the setter in the `impl<S: Space> Sim<S>` block:

```rust
    /// Attach dynamic land bridges (call once, after construction).
    pub fn set_bridges(&mut self, bridges: Bridges) {
        self.bridges = Some(bridges);
    }
```

- [ ] **Step 2: Drive bridges in `Sim::step`**

In `src/sim.rs` `step`, immediately after the `climate::apply_climate(...)` line and before `ecology::absorb(...)`, add:

```rust
        // Dynamic land bridges: open/close cells, drown anyone on a sunk cell.
        if self.bridges.is_some() {
            let upd = self.bridges.as_mut().unwrap().update(&self.calendar);
            for &i in &upd.opened {
                self.world.set_cell_passable(i, true);
            }
            for &i in &upd.closed {
                self.world.set_cell_passable(i, false);
            }
            ecology::drown(&self.world.space, &mut self.pop, &upd.closed);
        }
```

- [ ] **Step 3: Compute + attach bridges in `mapview::load_scene`**

In `src/bin/mapview.rs`, add imports:

```rust
use alife::bridges::{find_bridge_sites, BridgeConfig, Bridges};
use alife::rng::Rng;
```

In `load_scene`, replace the block from `let world = world_from_materials(...)` through the `Sim::new(...)` line with:

```rust
        let world = world_from_materials(sw, sh, &sim_mats);
        let mut sim = Sim::new(world, EcoParams::default(), 0xA11FE);
        let bcfg = BridgeConfig::default();
        let sites = find_bridge_sites(&sim_mats, sw, sh, &continents, &mut Rng::new(0xB12D6E), &bcfg);
        sim.set_bridges(Bridges::new(sites, bcfg, 0xB12D6E));
```

- [ ] **Step 4: Build, test, lint**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets && cargo build --bin mapview`
Expected: PASS, clippy clean, mapview builds.

- [ ] **Step 5: Manual smoke check**

Run: `cargo run --release --bin mapview`
Expected: the sim runs; during the Vraze arh, temporary land appears across some ocean straits between continents (organisms can be seen crossing), then it sinks again. (Bridges only appear on the textured `.tmx` path.)

- [ ] **Step 6: Commit**

```bash
git add src/sim.rs src/bin/mapview.rs
git commit -m "feat: drive land bridges from Sim::step and attach in mapview"
```

---

### Task 8: Document plan 5

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a disturbance note**

In `README.md`, in the bullet about the 6-arh year (the line ending "edge-of-chaos engine."), append a sentence:

```markdown
  valaar). Each season rewards a *different* adaptation — the edge-of-chaos engine.
  During **Vraze**, temporary **land bridges** rise across narrow straits (gene
  flow between continents); mutation rate is a **field** — fast in the valaar-rich
  core, slow in the Dusk, and spiking in **Rasgun**.
```

- [ ] **Step 2: Lint check and commit**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

```bash
git add README.md
git commit -m "docs: note Vraze land bridges + mutation field"
```

---

## Self-Review

**Spec coverage:**
- Periodic land bridges, computed at load, seeded-random, 3–6 wide → Task 1 (`find_bridge_sites`). ✓
- Vraze cadence, random subset/year, random offset + duration within Vraze → Task 2 (`Bridges::schedule`/`update`). ✓
- Open = passable, close = revert, drown occupants → Task 3 (`set_cell_passable`), Task 4 (`drown`), Task 7 (Sim integration). ✓
- Mutation-as-a-field: valaar-scaled with floor/ceil, Rasgun spike, applied at birth → Task 5 (`mutation_rate`), Task 6 (`reproduce`). ✓
- Determinism preserved (separate bridge RNG; unchanged reproduce draw count) → Global Constraints + Tasks 2/6. ✓
- Bridges terrain-path only; headless unaffected → Task 7 (Option), Sim default `None`. ✓

**Placeholder scan:** every code step shows full code; run steps give command + expected result; no TBD/TODO. ✓

**Type consistency:** `BridgeSite.cells: Vec<usize>`, `BridgeConfig` (Copy), `BridgeUpdate{opened,closed}`, `Bridges::{new(sites,cfg,seed), update(&Calendar)->BridgeUpdate, set_bridges}` consistent across Tasks 1/2/7; `find_bridge_sites(mats,sw,sh,labels,rng,cfg)` matches the `mapview` call (Task 7) and tests (Task 1); `World::set_cell_passable(usize,bool)` (Task 3) used in Task 7; `drown(space,pop,&[usize])` (Task 4) used in Task 7; `mutation_rate(eco,f32,Season)` (Task 5) used by `reproduce` (Task 6); `reproduce(space,valaar,pop,eco,rng,season)` matches the `Sim::step` call (Task 6). ✓

**Deferred (later plans):** the valaar state-machine (seasonal valaar forms) → **plan 5b**; the burrow / subterranean gene + underground as a climate refuge → **plan 6**. (Earthquake-opened underground access and sea-floor rise are dropped — no longer in scope.)
