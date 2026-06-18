# Climate + Seasons (the 6-arh cycle) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the living world a *climate* — heat and water fields that swing through the six arhs of the year — plus the two tolerance genes and the stress pressure that make those swings matter, so each season rewards a different adaptation (the edge-of-chaos engine).

**Architecture:** Layered on plan 1 (substrate) + plan 2 (organisms/ecology). Two new per-cell scalar `Field`s (heat, water) join the valaar field. A deterministic `Calendar` counts craws→arhs and reports the current `Season`; a `Climate` config maps each season to target heat/water levels (and a valaar-source multiplier). Each tick the climate fields relax toward the season's targets, a new `environmental_stress` ecology step drains energy from organisms whose `heat_tolerance`/`drought_tolerance` genes can't cope with their cell, and `Sim` weaves it all into the existing tick order. Still CPU, std-only, deterministic — no RNG added.

**Tech Stack:** Rust (edition 2021), std only.

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** `~/dev/ideas/world-design.md` §F (environmental axes ↔ genes; the 6-arh cycle: Rasgun→Goscon→Miscre→Vraze→Dansch→Laisp; hazards) and §D. Calendar: 6 arhs × 117 craws = 702 craws/year.

**Builds on (plans 1–2, shipped — exact interfaces consumed):**

- `genome::{Genome, TRAIT_COUNT, from_array, to_array, clamped, random, mutate}` — **extended here** (6→8 traits).
- `organism::{Organism (trait), TraitOrganism}`; `population::Population`; `params::EcoParams`.
- `ecology::{absorb, move_organisms, predate, metabolize, cull_and_recycle, reproduce}`.
- `sim::Sim { pub world, pub pop, pub eco, pub rng }` with `new(world, eco, seed)`, `step()`, `seed_organism`, `population()` — **extended here**.
- `field::Field` (`zeros,len,get,set,add,total,scale_all`); `space::{Coord, Layer, Space, Grid2p5D}`; `world::{World, Params}`; `rng::Rng`.

## Global Constraints

- **Language:** Rust, edition 2021. **No external crates** (std only).
- **Determinism:** no RNG added in this plan. Climate, the calendar, and stress are all deterministic functions of `(tick, params, fields)`. Identical seed + params ⇒ identical run.
- **Spatial access stays behind `Space`:** heat/water are `Field`s indexed via `Space::index`; no module outside `space.rs` assumes a layout.
- **Every axis has a matching gene (world-design §C):** heat ↔ `heat_tolerance`, water ↔ `drought_tolerance`. This plan adds *exactly* those two genes — no gene without a field, no field without a gene.
- **Implicit selection only:** stress drains energy (→ starvation → death via the existing cull). Nothing scores fitness.
- **Valaar stays the one currency:** heat/water are *stress axes*, not food. They never add energy; they only impose costs. Organisms still eat only valaar.
- **Every rate is a dial:** all climate/stress constants live on `Climate` (season targets) and `EcoParams` (stress coefficients).
- **Backward compatibility:** plan 2's behaviour is preserved when climate is neutral. The two new genes are appended at genome indices 6–7, so trait indices 0–5 keep their meaning.
- **Version control:** commit per task, **no `Co-Authored-By` trailer** (user preference).

**Scope (plan 3 of several) — what's DEFERRED, and to which plan:**

**Plan 4 (the agreed next plan) — static geography.** Hard-wire the world's *abiotic stage* from the Alchaea lore: continents, oceans (barriers), and the valaar rivers flowing from Rasconne — as a per-cell **terrain map** (cell type + valaar conductivity + passability) that the existing diffusion (`diffuse_planar`) and movement (`move_organisms`) become aware of. Authoring this is *correct*, not a compromise: geography is geological backstory, fixed on evolutionary timescales, so it's the stage; biology still emerges on top. It is also the prerequisite for the dynamic-world work below (land-bridges need oceans to bridge) and for real **speciation** (oceans = barriers → allopatric divergence).
  - **Workflow:** the user sketches a rough 2D structure; a **lore-constrained procedural generator** fleshes it into the terrain map (continents split outward from a central Rasconne, oceans between, valaar rivers traced down the gradient) — seeded/reproducible, with a canonical seed = "the" Alchaea.
  - **Visualisation (likely required at this point):** once a static map exists we need a renderer — at minimum to eyeball that the map *makes sense*. 2D → a grid/tile view; a future 3D world → voxel-style. This is the project's first real renderer (beyond the ASCII heatmap).

**Plan 5+ — the dynamic world (depends on plan 4's geography):**

- Dynamic geography: Vraze **land-bridges** + **earthquakes** opening underground access (mutating the terrain / access-points over time).
- Valaar **state machine**: Vraze crystallisation, Miscre **airborne valaar reaching the Dusk** (valaar↔climate cross-coupling).
- **Mutation-as-a-field** (local mutation rate ∝ valaar, spiking at Rasgun) — still *pending the user's nod*.
- The underground as a **climate refuge** + a `digging`/`subterranean` gene (no layer-changing movement exists yet).

---

## Design assumptions (flagged for veto)

- **A1 — two new genes only:** `heat_tolerance`, `drought_tolerance`, appended at genome indices 6, 7 (genome now 8 traits). No `digging` gene yet (no layer movement to use it).
- **A2 — heat/water are uniform per season (no spatial gradient yet):** the whole map relaxes toward one season target. Enough to exercise the genes (the hot/dry season stresses everyone). Spatial climate (latitude, altitude) is future.
- **A3 — climate applies to both layers equally:** underground isn't a refuge yet (organisms can't dig there), so the simplification is invisible. Underground buffering arrives with the `digging` gene (plan 4).
- **A4 — stress formula:** `heat_penalty = heat_stress·max(0, heat − heat_tolerance)`; `drought_penalty = drought_stress·max(0, (1 − drought_tolerance) − water)`. So a high `heat_tolerance` shrugs off heat; a high `drought_tolerance` lowers the water it *needs*. Energy is drained, never added.
- **A5 — season → environment table** (the numbers in `Climate::default`, Task 3): a concrete, tunable mapping. The wet peak is Miscre (monsoon) and the dry peak is Dansch (drought) per the lore; the **heat curve and the valaar multipliers are my engineering guess** — adjust freely. The *season names and their character are your lore*; I only assigned numbers.
- **A6 — valaar–season coupling:** each season multiplies the Rasconne `source_rate` (Rasgun surge … Laisp crash). A light touch so valaar abundance breathes with the year.
- **A7 — tick order:** `advance calendar → set season's valaar multiplier → world.step() → relax climate fields → absorb → move → predate → environmental_stress → metabolize → cull_and_recycle → reproduce`. Stress sits just before metabolism (both are energy drains resolved before death).
- **A8 — `Sim::new` keeps its 3-arg shape** (`world, eco, seed`) and defaults `Climate`; a second constructor `Sim::with_climate(world, eco, climate, seed)` injects a custom climate. Plan 2's integration test keeps compiling; climate runs with gentle defaults.

---

## File Structure

- `src/genome.rs` — **modified**: 6→8 traits (`heat_tolerance`, `drought_tolerance`).
- `src/season.rs` — **new**: `Season` enum + `Calendar` (craw/arh counter).
- `src/climate.rs` — **new**: `Climate` config (per-season targets + relax rate) + `apply_climate` (relax heat/water fields toward the season target) + the season→target lookup.
- `src/ecology.rs` — **modified**: add `environmental_stress`.
- `src/params.rs` — **modified**: add `heat_stress` + `drought_stress` coefficients to `EcoParams`.
- `src/sim.rs` — **modified**: `Sim` gains `heat`, `water`, `calendar`, `climate`; `step()` weaves them in; `with_climate` constructor.
- `src/bin/life.rs` — **modified**: census prints the season + mean tolerance genes.
- `tests/ecology.rs` — **modified**: keep persistence green with climate on; add a season-cycles + stress-selects test.

Each `src/lib.rs` edit adds one `pub mod`, shown in its task.

---

### Task 1: Extend the genome to 8 traits

**Files:**

- Modify: `src/genome.rs` (consts, struct, `from_array`/`to_array`, tests)
- Modify: `src/organism.rs`, `src/population.rs`, `src/ecology.rs`, `tests/ecology.rs`, `src/bin/life.rs` (fixture call sites — append two trait values)

**Interfaces:**

- Consumes: `Rng` (plan 1).
- Produces: `pub const TRAIT_COUNT: usize = 8;` and `Genome` with two extra fields `pub heat_tolerance: f32, pub drought_tolerance: f32` (indices 6, 7). `random`/`mutate`/`clamped` are unchanged (they loop over `TRAIT_COUNT`).

- [ ] **Step 1: Update the genome tests for 8 traits**

In `src/genome.rs`, replace the `round_trips_through_array` and `mutate_with_zero_rate_is_identity` test bodies (the ones with 6-element arrays) and extend the extreme-mutation array:

```rust
    #[test]
    fn round_trips_through_array() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
        assert_eq!(g.to_array(), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
        assert_eq!(g.diet, 0.4);
        assert_eq!(g.heat_tolerance, 0.7);
        assert_eq!(g.drought_tolerance, 0.8);
    }
```

In `clamped_pins_to_unit_interval`, extend the array to 8: `[-1.0, 2.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]`.
In `mutate_stays_in_bounds_even_from_extremes`, extend to `[0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]`.
In `mutate_with_zero_rate_is_identity`, extend to `[0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.2, 0.9]`.

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib genome`
Expected: **compile error** — arrays of length 8 don't match `from_array(a: [f32; 6])` / fields `heat_tolerance` not found.

- [ ] **Step 3: Extend the `Genome` struct and array bridges**

In `src/genome.rs`, change the constant and struct and the two array methods:

```rust
/// Number of scalar traits in the genome.
pub const TRAIT_COUNT: usize = 8;
```

Add the two fields to `Genome` (after `lifespan`):

```rust
    /// Maps (via EcoParams) to a maximum age in ticks.
    pub lifespan: f32,
    /// Resistance to heat stress (0 = none, 1 = immune within range).
    pub heat_tolerance: f32,
    /// Resistance to drought (high = needs little water).
    pub drought_tolerance: f32,
```

Update `from_array`:

```rust
    pub fn from_array(a: [f32; TRAIT_COUNT]) -> Self {
        Genome {
            size: a[0],
            valaar_efficiency: a[1],
            speed: a[2],
            diet: a[3],
            repro_threshold: a[4],
            lifespan: a[5],
            heat_tolerance: a[6],
            drought_tolerance: a[7],
        }
    }
```

Update `to_array`:

```rust
    pub fn to_array(&self) -> [f32; TRAIT_COUNT] {
        [
            self.size,
            self.valaar_efficiency,
            self.speed,
            self.diet,
            self.repro_threshold,
            self.lifespan,
            self.heat_tolerance,
            self.drought_tolerance,
        ]
    }
```

(`clamped`, `random`, `mutate` need no change — they iterate `to_array()`/`TRAIT_COUNT`.)

- [ ] **Step 4: Run the genome tests**

Run: `cd ~/dev/alife && cargo test --lib genome`
Expected: `test result: ok. 5 passed`.

- [ ] **Step 5: Fix every downstream fixture (append two trait values)**

The whole crate's `from_array([...; 6])` call sites now fail to compile. Update each by **appending `0.5, 0.5`** (neutral tolerances) unless noted:

In `src/organism.rs` test helper:

```rust
    fn genome_with(size: f32, lifespan: f32) -> Genome {
        Genome::from_array([size, 0.5, 0.5, 0.5, 0.5, lifespan, 0.5, 0.5])
    }
```

In `src/population.rs` test helper: `Genome::from_array([0.5; 6])` → `Genome::from_array([0.5; 8])`.

In `src/ecology.rs` tests:

```rust
    fn genome(diet: f32, eff: f32) -> Genome {
        Genome::from_array([0.5, eff, 0.0, diet, 0.9, 0.5, 0.5, 0.5])
    }
    fn predator(size: f32) -> Genome {
        Genome::from_array([size, 1.0, 0.0, 1.0, 0.9, 0.5, 0.5, 0.5])
    }
    fn prey(size: f32) -> Genome {
        Genome::from_array([size, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5])
    }
```

In `src/ecology.rs` movement tests, both `Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5])` → `[0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5]`.
In `src/ecology.rs` reproduce tests: `[0.5, 1.0, 0.0, 0.0, 0.0, 0.5]` → `[0.5, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5]` and `[0.5, 1.0, 0.0, 0.0, 1.0, 0.5]` → `[0.5, 1.0, 0.0, 0.0, 1.0, 0.5, 0.5, 0.5]`.

In `tests/ecology.rs` (`seeded_sim`): `Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6])` → `[0.4, 0.7, 0.3, 0.2, 0.5, 0.6, 0.6, 0.6]` (decent tolerances so seeds survive their first seasons).

In `src/bin/life.rs`: `Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6])` → `[0.4, 0.7, 0.3, 0.2, 0.5, 0.6, 0.6, 0.6]`.

- [ ] **Step 6: Run the whole suite**

Run: `cd ~/dev/alife && cargo test`
Expected: all green (the 47 unit + 2 integration + 1 gradient tests still pass; climate isn't wired in yet so behaviour is unchanged).

- [ ] **Step 7: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: genome gains heat_tolerance + drought_tolerance (8 traits)"
```

---

### Task 2: The season calendar (`season`)

**Files:**

- Create: `src/season.rs`
- Modify: `src/lib.rs` (add `pub mod season;`)

**Interfaces:**

- Consumes: nothing.
- Produces:
  - `pub const CRAWS_PER_ARH: u32 = 117;` `pub const ARHS_PER_YEAR: u32 = 6;` `pub const CRAWS_PER_YEAR: u32 = 702;`
  - `pub enum Season { Rasgun, Goscon, Miscre, Vraze, Dansch, Laisp }` with `pub const ALL: [Season; 6]` and `pub fn index(self) -> usize`.
  - `pub struct Calendar { /* private craw */ }` with `pub fn new() -> Calendar` (+ `Default`), `pub fn advance(&mut self)`, `pub fn craw(&self) -> u32` (craw of the current year, `0..702`), `pub fn arh(&self) -> u32` (`0..6`), `pub fn year(&self) -> u32`, `pub fn season(&self) -> Season`.

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod season;
```

- [ ] **Step 2: Write the failing tests**

Create `src/season.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_has_six_arhs_of_117_craws() {
        assert_eq!(CRAWS_PER_ARH, 117);
        assert_eq!(ARHS_PER_YEAR, 6);
        assert_eq!(CRAWS_PER_YEAR, 702);
        assert_eq!(Season::ALL.len(), 6);
    }

    #[test]
    fn calendar_starts_at_rasgun() {
        let c = Calendar::new();
        assert_eq!(c.craw(), 0);
        assert_eq!(c.arh(), 0);
        assert_eq!(c.year(), 0);
        assert_eq!(c.season(), Season::Rasgun);
    }

    #[test]
    fn arh_boundary_advances_the_season() {
        let mut c = Calendar::new();
        for _ in 0..CRAWS_PER_ARH {
            c.advance();
        }
        assert_eq!(c.arh(), 1);
        assert_eq!(c.season(), Season::Goscon);
        assert_eq!(c.craw(), CRAWS_PER_ARH);
    }

    #[test]
    fn year_wraps_after_all_six_arhs() {
        let mut c = Calendar::new();
        for _ in 0..CRAWS_PER_YEAR {
            c.advance();
        }
        assert_eq!(c.year(), 1);
        assert_eq!(c.craw(), 0);
        assert_eq!(c.season(), Season::Rasgun);
    }

    #[test]
    fn seasons_follow_the_lore_order() {
        let order = [
            Season::Rasgun,
            Season::Goscon,
            Season::Miscre,
            Season::Vraze,
            Season::Dansch,
            Season::Laisp,
        ];
        let mut c = Calendar::new();
        for (arh, &want) in order.iter().enumerate() {
            // jump to the middle of arh `arh`
            let target = arh as u32 * CRAWS_PER_ARH + 10;
            while c.craw() < target {
                c.advance();
            }
            assert_eq!(c.season(), want, "arh {arh}");
        }
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib season`
Expected: **compile error** — `cannot find type Calendar` / `Season`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/season.rs`:

```rust
//! The 6-arh year. A deterministic clock: `Calendar` counts craws (days),
//! groups them into arhs (months), and reports the current `Season`. Seasons
//! drive the climate (see `climate.rs`).

/// Craws (days) per arh (month).
pub const CRAWS_PER_ARH: u32 = 117;
/// Arhs (months) per year.
pub const ARHS_PER_YEAR: u32 = 6;
/// Craws per full year.
pub const CRAWS_PER_YEAR: u32 = CRAWS_PER_ARH * ARHS_PER_YEAR;

/// The six arhs, in order. Each rewards a different adaptation.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Season {
    Rasgun,
    Goscon,
    Miscre,
    Vraze,
    Dansch,
    Laisp,
}

impl Season {
    pub const ALL: [Season; 6] = [
        Season::Rasgun,
        Season::Goscon,
        Season::Miscre,
        Season::Vraze,
        Season::Dansch,
        Season::Laisp,
    ];

    pub fn index(self) -> usize {
        match self {
            Season::Rasgun => 0,
            Season::Goscon => 1,
            Season::Miscre => 2,
            Season::Vraze => 3,
            Season::Dansch => 4,
            Season::Laisp => 5,
        }
    }
}

/// Deterministic day counter. `craw` is the day within the current year.
#[derive(Clone, Debug, Default)]
pub struct Calendar {
    craw: u32,
    year: u32,
}

impl Calendar {
    pub fn new() -> Self {
        Calendar { craw: 0, year: 0 }
    }

    /// Advance one craw (one tick), wrapping the year.
    pub fn advance(&mut self) {
        self.craw += 1;
        if self.craw >= CRAWS_PER_YEAR {
            self.craw = 0;
            self.year += 1;
        }
    }

    pub fn craw(&self) -> u32 {
        self.craw
    }

    pub fn arh(&self) -> u32 {
        self.craw / CRAWS_PER_ARH
    }

    pub fn year(&self) -> u32 {
        self.year
    }

    pub fn season(&self) -> Season {
        Season::ALL[self.arh() as usize]
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib season`
Expected: `test result: ok. 5 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: 6-arh season calendar"
```

---

### Task 3: Climate config + relaxation (`climate`)

**Files:**

- Create: `src/climate.rs`
- Modify: `src/lib.rs` (add `pub mod climate;`)

**Interfaces:**

- Consumes: `Field` (plan 1), `Season` (Task 2).
- Produces:
  - `pub struct SeasonTarget { pub heat: f32, pub water: f32, pub valaar_mult: f32 }`.
  - `pub struct Climate { pub targets: [SeasonTarget; 6], pub relax_rate: f32 }` + `impl Default`.
  - `pub fn target(climate: &Climate, season: Season) -> SeasonTarget`.
  - `pub fn relax_field(field: &mut Field, target: f32, rate: f32)`.
  - `pub fn apply_climate(heat: &mut Field, water: &mut Field, season: Season, climate: &Climate)`. `[ASSUMPTION A5]`

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod climate;
```

- [ ] **Step 2: Write the failing tests**

Create `src/climate.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Field;
    use crate::season::Season;

    #[test]
    fn dansch_is_the_drought_and_miscre_the_monsoon() {
        let c = Climate::default();
        let dansch = target(&c, Season::Dansch);
        let miscre = target(&c, Season::Miscre);
        assert!(dansch.water < miscre.water, "Dansch should be drier than Miscre");
        assert!(dansch.water < 0.3, "Dansch is a drought");
        assert!(miscre.water > 0.7, "Miscre is a monsoon");
    }

    #[test]
    fn relax_moves_a_field_toward_target() {
        let mut f = Field::zeros(4);
        for _ in 0..200 {
            relax_field(&mut f, 0.8, 0.05);
        }
        for i in 0..f.len() {
            assert!((f.get(i) - 0.8).abs() < 1e-3, "cell {i} = {}", f.get(i));
        }
    }

    #[test]
    fn apply_climate_pushes_both_fields_to_the_season() {
        let climate = Climate::default();
        let mut heat = Field::zeros(4);
        let mut water = Field::zeros(4);
        let t = target(&climate, Season::Dansch);
        for _ in 0..300 {
            apply_climate(&mut heat, &mut water, Season::Dansch, &climate);
        }
        assert!((heat.get(0) - t.heat).abs() < 1e-2);
        assert!((water.get(0) - t.water).abs() < 1e-2);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib climate`
Expected: **compile error** — `cannot find type Climate` / function `target`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/climate.rs`:

```rust
//! Climate: the seasons drive per-cell heat and water. Each season has target
//! levels the fields relax toward; the genome's `heat_tolerance` /
//! `drought_tolerance` decide who can stand them (see `ecology::environmental_stress`).
//!
//! v1 is spatially uniform (one target for the whole map) — enough to make each
//! season select a different adaptation. Spatial climate is future work.

use crate::field::Field;
use crate::season::Season;

/// Where a season pushes the environment. `heat`/`water` in `[0, 1]`;
/// `valaar_mult` scales the Rasconne source for the season.
#[derive(Clone, Copy, Debug)]
pub struct SeasonTarget {
    pub heat: f32,
    pub water: f32,
    pub valaar_mult: f32,
}

/// The per-season climate table plus how fast fields chase their target.
#[derive(Clone, Debug)]
pub struct Climate {
    /// Indexed by `Season::index`.
    pub targets: [SeasonTarget; 6],
    /// Fraction of the gap closed per tick (`< 1`).
    pub relax_rate: f32,
}

impl Default for Climate {
    fn default() -> Self {
        // [ASSUMPTION A5] Wet peak = Miscre, dry peak = Dansch (lore). Heat curve
        // and valaar multipliers are an engineering guess — tune freely.
        // Order: Rasgun, Goscon, Miscre, Vraze, Dansch, Laisp.
        Climate {
            targets: [
                SeasonTarget { heat: 0.30, water: 0.60, valaar_mult: 1.5 }, // Rasgun: surge, spring
                SeasonTarget { heat: 0.50, water: 0.60, valaar_mult: 1.2 }, // Goscon: stable, peak
                SeasonTarget { heat: 0.40, water: 0.90, valaar_mult: 1.0 }, // Miscre: monsoon, fog
                SeasonTarget { heat: 0.60, water: 0.40, valaar_mult: 0.9 }, // Vraze: quakes, crystalline
                SeasonTarget { heat: 0.80, water: 0.15, valaar_mult: 0.6 }, // Dansch: drought, hot
                SeasonTarget { heat: 0.60, water: 0.30, valaar_mult: 0.4 }, // Laisp: lowest valaar
            ],
            relax_rate: 0.05,
        }
    }
}

/// The target for a given season.
pub fn target(climate: &Climate, season: Season) -> SeasonTarget {
    climate.targets[season.index()]
}

/// Ease every cell of `field` a `rate` fraction toward `target`.
pub fn relax_field(field: &mut Field, target: f32, rate: f32) {
    for i in 0..field.len() {
        let v = field.get(i);
        field.set(i, v + rate * (target - v));
    }
}

/// Relax both climate fields toward the current season's targets.
pub fn apply_climate(heat: &mut Field, water: &mut Field, season: Season, climate: &Climate) {
    let t = target(climate, season);
    relax_field(heat, t.heat, climate.relax_rate);
    relax_field(water, t.water, climate.relax_rate);
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib climate`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: climate config + season-driven field relaxation"
```

---

### Task 4: Environmental stress (`ecology::environmental_stress`)

**Files:**

- Modify: `src/params.rs` (add two stress coefficients)
- Modify: `src/ecology.rs` (add `environmental_stress`; extend `tests`)

**Interfaces:**

- Consumes: `Space`, `Field` (plan 1); `Population`, `EcoParams` (plan 2); the `Organism` trait for `pos`.
- Produces:
  - `EcoParams` gains `pub heat_stress: f32` and `pub drought_stress: f32` (with defaults).
  - `pub fn environmental_stress<S: Space>(space: &S, heat: &Field, water: &Field, pop: &mut Population, eco: &EcoParams)`. `[ASSUMPTION A4]`

- [ ] **Step 1: Add the stress coefficients to `EcoParams`**

In `src/params.rs`, add two fields to `EcoParams` (after `predation_efficiency`):

```rust
    /// Fraction of prey energy a predator captures.
    pub predation_efficiency: f32,
    /// Energy drained per unit of heat above an organism's heat_tolerance.
    pub heat_stress: f32,
    /// Energy drained per unit of water deficit below its need.
    pub drought_stress: f32,
```

And in `impl Default for EcoParams`, after `predation_efficiency: 0.6,`:

```rust
            predation_efficiency: 0.6,
            heat_stress: 0.1,
            drought_stress: 0.1,
```

- [ ] **Step 2: Write the failing tests**

Add inside the `tests` module in `src/ecology.rs` (the helpers `genome`, `predator`, `prey` already exist there; the `Field`/`Coord`/`Layer`/`Space` imports too):

```rust
    // genome with explicit tolerances: [size, eff, speed, diet, repro, lifespan, heat_tol, drought_tol]
    fn tol_genome(heat_tol: f32, drought_tol: f32) -> Genome {
        Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, heat_tol, drought_tol])
    }

    #[test]
    fn heat_intolerant_loses_energy_in_a_hot_cell() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut heat = crate::field::Field::zeros(space.len());
        let water = crate::field::Field::zeros(space.len());
        heat.set(space.index(c), 1.0); // scorching
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(tol_genome(0.1, 1.0), c, 5.0)); // can't take heat, no drought issue

        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert!(pop.organisms()[0].energy < 5.0, "heat-intolerant should suffer");
    }

    #[test]
    fn heat_tolerant_is_unscathed() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut heat = crate::field::Field::zeros(space.len());
        let mut water = crate::field::Field::zeros(space.len());
        heat.set(space.index(c), 1.0);
        water.set(space.index(c), 1.0); // wet, so no drought stress either
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(tol_genome(1.0, 1.0), c, 5.0)); // immune

        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 5.0, "tolerant should be unscathed");
    }

    #[test]
    fn drought_drains_the_intolerant() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let heat = crate::field::Field::zeros(space.len()); // cool
        let water = crate::field::Field::zeros(space.len()); // bone dry (0.0)
        let mut pop = Population::new();
        // drought_tolerance 0.0 => needs water 1.0; finds 0.0 => big deficit
        pop.spawn(TraitOrganism::new(tol_genome(1.0, 0.0), c, 5.0));

        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert!(pop.organisms()[0].energy < 5.0, "drought-intolerant should suffer");
    }
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `cannot find function environmental_stress`.

- [ ] **Step 4: Write the minimal implementation**

Add to `src/ecology.rs` (below `reproduce`, above the `tests` module):

```rust
/// Drain energy from organisms whose cell is hotter or drier than their genes
/// can stand. Heat above `heat_tolerance` and water below the organism's need
/// (`1 - drought_tolerance`) each cost energy. Never adds energy; deaths fall
/// out of the normal cull.
pub fn environmental_stress<S: Space>(
    space: &S,
    heat: &Field,
    water: &Field,
    pop: &mut Population,
    eco: &EcoParams,
) {
    for o in pop.organisms_mut() {
        let i = space.index(o.pos);
        let heat_excess = (heat.get(i) - o.genome.heat_tolerance).max(0.0);
        let water_need = 1.0 - o.genome.drought_tolerance;
        let water_deficit = (water_need - water.get(i)).max(0.0);
        let penalty = eco.heat_stress * heat_excess + eco.drought_stress * water_deficit;
        o.energy -= penalty;
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: `test result: ok. 16 passed` (13 from plan 2 + 3 new).

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: environmental_stress drains heat/drought-intolerant organisms"
```

---

### Task 5: Wire climate into `Sim`

**Files:**

- Modify: `src/sim.rs` (new fields, `with_climate`, `step` order)
- Modify: `tests/ecology.rs` (keep the existing persistence test green with climate on)

**Interfaces:**

- Consumes: everything above + plan 1/2.
- Produces: `Sim` gains `pub heat: Field, pub water: Field, pub calendar: Calendar, pub climate: Climate`; `pub fn with_climate(world: World<S>, eco: EcoParams, climate: Climate, seed: u64) -> Sim<S>`; `pub fn season(&self) -> Season`; `Sim::new` now also initialises those (defaulting `Climate`). `[ASSUMPTION A6, A7, A8]`

- [ ] **Step 1: Update `Sim` and its `step`**

In `src/sim.rs`, extend the imports:

```rust
use crate::climate::{self, Climate};
use crate::ecology;
use crate::field::Field;
use crate::organism::TraitOrganism;
use crate::params::EcoParams;
use crate::population::Population;
use crate::rng::Rng;
use crate::season::{Calendar, Season};
use crate::space::Space;
use crate::world::World;
```

Replace the `Sim` struct and its `impl` with:

```rust
pub struct Sim<S: Space> {
    pub world: World<S>,
    pub pop: Population,
    pub eco: EcoParams,
    pub rng: Rng,
    pub heat: Field,
    pub water: Field,
    pub calendar: Calendar,
    pub climate: Climate,
    /// Un-multiplied Rasconne source rate, captured at construction so the
    /// per-season multiplier always scales the same base.
    base_source: f32,
}

impl<S: Space> Sim<S> {
    /// Build a sim with the default climate.
    pub fn new(world: World<S>, eco: EcoParams, seed: u64) -> Self {
        Sim::with_climate(world, eco, Climate::default(), seed)
    }

    /// Build a sim with a chosen climate.
    pub fn with_climate(world: World<S>, eco: EcoParams, climate: Climate, seed: u64) -> Self {
        let len = world.space.len();
        let base_source = world.params.source_rate;
        Sim {
            world,
            pop: Population::new(),
            eco,
            rng: Rng::new(seed),
            heat: Field::zeros(len),
            water: Field::zeros(len),
            calendar: Calendar::new(),
            climate,
            base_source,
        }
    }

    /// Current number of living organisms.
    pub fn population(&self) -> usize {
        self.pop.len()
    }

    /// The current season.
    pub fn season(&self) -> Season {
        self.calendar.season()
    }

    /// Place an organism into the world (used for the initial seeding).
    pub fn seed_organism(&mut self, o: TraitOrganism) {
        self.pop.spawn(o);
    }

    /// Advance one tick: calendar → season-coupled valaar → substrate → climate
    /// → ecology loop (with environmental stress before metabolism).
    pub fn step(&mut self) {
        self.calendar.advance();
        let season = self.calendar.season();

        // [A6] valaar abundance breathes with the season.
        self.world.params.source_rate =
            self.base_source * climate::target(&self.climate, season).valaar_mult;

        self.world.step();
        climate::apply_climate(&mut self.heat, &mut self.water, season, &self.climate);

        ecology::absorb(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
        );
        ecology::predate(&self.world.space, &mut self.pop, &self.eco);
        ecology::environmental_stress(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
        ecology::cull_and_recycle(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::reproduce(&mut self.pop, &self.eco, &mut self.rng);
    }
}

- [ ] **Step 2: Keep the existing integration test compiling/green**

`tests/ecology.rs` already calls `Sim::new(world, eco, seed)` and `sim.world.step()` for warmup — both still valid. No change needed for it to compile. Run it:

Run: `cd ~/dev/alife && cargo test --test ecology`
Expected: `test result: ok. 2 passed`. If `life_persists_under_the_gradient` now fails (climate stress tipped it to extinction), raise the seed organisms' tolerances in `seeded_sim` (already `0.6, 0.6` from Task 1 Step 5) toward `0.8` and/or shorten the run — note it as an `EcoParams`/`Climate` tuning signal (`[A5]`), not a code bug.

- [ ] **Step 3: Run the whole suite**

Run: `cd ~/dev/alife && cargo test`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: Sim weaves calendar + climate + stress into the tick"
```

---

### Task 6: Season-aware census + integration tests

**Files:**

- Modify: `src/bin/life.rs` (print season + mean tolerances)
- Modify: `tests/ecology.rs` (add season-cycle + stress-selection tests)

**Interfaces:**

- Consumes: `Sim`, `Season`, `Climate`, `Calendar`, plan 1/2.
- Produces: no new public API — verification + demonstration.

- [ ] **Step 1: Write the failing integration tests**

Add to `tests/ecology.rs` (extend the imports with `use alife::season::Season;`):

```rust
#[test]
fn the_year_cycles_through_all_seasons() {
    let mut sim = seeded_sim(3);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..alife::season::CRAWS_PER_YEAR {
        seen.insert(sim.season());
        sim.step();
    }
    // every arh should appear across a full year
    assert_eq!(seen.len(), 6, "all six seasons should occur in a year");
}

#[test]
fn drought_selects_for_drought_tolerance() {
    use alife::genome::Genome;
    use alife::organism::TraitOrganism;
    use alife::params::EcoParams;
    use alife::space::{Coord, Grid2p5D, Layer};
    use alife::world::{Params, World};

    // A tiny world held permanently in Dansch (drought) by a custom climate:
    // every season targets the same bone-dry, hot environment.
    let space = Grid2p5D::new(8, 8);
    let mut world = World::new(space, Params::default());
    world.add_source(Coord::new(4, 4, Layer::Surface));
    let dry = alife::climate::SeasonTarget { heat: 0.9, water: 0.0, valaar_mult: 1.0 };
    let climate = alife::climate::Climate { targets: [dry; 6], relax_rate: 0.2 };
    let mut sim = Sim::with_climate(world, EcoParams::default(), climate, 1);
    for _ in 0..100 {
        sim.world.step();
    }
    // Two co-located lineages: one drought/heat tolerant, one not.
    for _ in 0..20 {
        let tough = Genome::from_array([0.3, 0.8, 0.1, 0.0, 0.4, 0.6, 0.9, 0.9]);
        let frail = Genome::from_array([0.3, 0.8, 0.1, 0.0, 0.4, 0.6, 0.1, 0.1]);
        sim.seed_organism(TraitOrganism::new(tough, Coord::new(4, 4, Layer::Surface), sim.eco.initial_energy));
        sim.seed_organism(TraitOrganism::new(frail, Coord::new(4, 4, Layer::Surface), sim.eco.initial_energy));
    }
    for _ in 0..200 {
        sim.step();
    }
    // Mean drought_tolerance should rise above the 0.5 we started balanced at.
    let n = sim.pop.len().max(1) as f32;
    let mean_dt: f32 = sim.pop.organisms().iter().map(|o| o.genome.drought_tolerance).sum::<f32>() / n;
    assert!(sim.pop.len() > 0, "population went extinct");
    assert!(mean_dt > 0.5, "drought should have selected for tolerance, got {mean_dt}");
}
```

- [ ] **Step 2: Run to verify they fail/compile-fail appropriately**

Run: `cd ~/dev/alife && cargo test --test ecology`
Expected: compile error first (`Season` not imported / `SeasonTarget` path), then once compiling, `the_year_cycles_through_all_seasons` and `drought_selects_for_drought_tolerance` run. If `drought_selects...` fails on the `mean_dt` threshold, treat as a `Climate`/`EcoParams` tuning signal: increase `relax_rate`, `drought_stress`, the run length, or the seed count until selection is detectable — the *direction* (mean_dt rises) is the invariant, the exact value is regime-dependent. (Determinism still holds: the test is seed-fixed.)

- [ ] **Step 3: Make them pass**

These need no new production code — they exercise Task 5. Adjust only the test constants per the tuning note until both pass. Confirm:

Run: `cd ~/dev/alife && cargo test --test ecology`
Expected: `test result: ok. 4 passed`.

- [ ] **Step 4: Extend the `life` census with the season + tolerances**

In `src/bin/life.rs`, replace the header line and the per-tick census block so it also reports the season and mean tolerance genes:

```rust
    println!("tick  season   pop   mean_size mean_diet mean_heat_tol mean_drought_tol");
    for tick in 0..=alife::season::CRAWS_PER_YEAR * 2 {
        if tick % 50 == 0 {
            let n = sim.pop.len().max(1) as f32;
            let mut s = 0.0;
            let mut d = 0.0;
            let mut ht = 0.0;
            let mut dt = 0.0;
            for o in sim.pop.organisms() {
                s += o.genome.size;
                d += o.genome.diet;
                ht += o.genome.heat_tolerance;
                dt += o.genome.drought_tolerance;
            }
            println!(
                "{tick:>4}  {:>7?}  {:>4}   {:>8.3} {:>8.3} {:>12.3} {:>15.3}",
                sim.season(),
                sim.pop.len(),
                s / n,
                d / n,
                ht / n,
                dt / n,
            );
        }
        sim.step();
    }
```

(`{:?}` formats the `Season` enum via its `Debug` derive.)

- [ ] **Step 5: Run the whole suite and the runner**

Run: `cd ~/dev/alife && cargo test`
Expected: all green — plan 1 (18+1), plan 2 + plan 3 unit tests, and `tests/ecology.rs` (4 passed).

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: no warnings.

Run: `cd ~/dev/alife && cargo run --bin life`
Expected: a census table whose `season` column cycles Rasgun→…→Laisp and whose `mean_heat_tol` / `mean_drought_tol` drift upward as the hot/dry seasons cull the intolerant.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: season-aware life census + climate selection integration tests"
```

---

## What this delivers

A world with weather: heat and water fields that cycle through the six arhs, two tolerance genes under genuine seasonal selection, and a calendar that makes Dansch a drought to survive and Miscre a monsoon — all deterministic, std-only, and layered cleanly on the existing substrate and ecology. Each season now rewards a different organism, which is the edge-of-chaos engine the design is built around.

**Next plans (not in scope here):** **Plan 4 — static geography** (continents, oceans, valaar rivers as a terrain map; built from a user sketch via a lore-constrained procedural generator; needs a map visualiser — grid/tile for 2D, voxel-style for 3D — to validate it). Then **plan 5+ — the dynamic world**: dynamic geography (Vraze land-bridges + earthquakes); the valaar state-machine (crystallisation, Miscre airborne valaar → Dusk); the `digging`/subterranean gene + underground refuge; mutation-as-a-field (pending a nod). Cross-cutting, later: wgpu acceleration + the live renderer; the Python natural-history / timeline layer.

## Self-review notes

- **Spec coverage:** environmental axes ↔ genes (Task 1, A1); the 6-arh calendar (Task 2); season-driven climate incl. monsoon/drought hazards (Task 3, A5); the tolerance selection pressure (Task 4, A4); season↔valaar coupling (Task 5, A6); demonstration of cyclic seasons + selection (Task 6). Geography-mutation / valaar-state / mutation-field explicitly deferred.
- **Type consistency:** genome trait order is `[size, valaar_efficiency, speed, diet, repro_threshold, lifespan, heat_tolerance, drought_tolerance]` everywhere (Task 1 and all fixtures). `SeasonTarget` fields `{heat, water, valaar_mult}` and `Climate` fields `{targets, relax_rate}` are used identically in `climate.rs`, `sim.rs`, and the Task 6 test. `Sim`'s new fields (`heat, water, calendar, climate, base_source`) match their uses in `step`.
- **Determinism:** no new RNG; climate/calendar/stress are pure functions of tick and params; organisms processed in `Vec` order; the new integration tests are seed-fixed.
