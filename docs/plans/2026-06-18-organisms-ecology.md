# Organisms + Ecology Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Implementation note (2026-06-18, post-write — this plan is now ✅ built):** Two deviations from the
> text below were applied during execution. (1) **Rename:** the concrete organism is **`TraitOrganism`**,
> implementing a clade-agnostic **`Organism` trait** (so `NeuralOrganism` / `MorphologicalOrganism` can be
> added later). Read every `Organism` *struct* reference below as `TraitOrganism`; the derived methods
> (`max_energy` / `basal_cost` / `lifespan_ticks` / `is_alive`) live on the `Organism` *trait*, which the
> ecology/population modules import. (2) **Task 6 fix:** the `metabolize` test seeds energy `2.0`, not
> `5.0` — a `5.0` seed exceeds size-0.5 storage (`max_energy` = 3.0) and the storage cap masked the basal
> subtraction.

**Goal:** Put *life* on the world substrate — trait-vector organisms that absorb valaar, move, eat each other, age, die back into the field, and reproduce with mutation — so an unguided population persists and evolves under the emergent valaar gradient.

**Architecture:** Layered on top of plan 1's substrate. New modules: a std-only seeded PRNG (`rng`), the `genome` trait-vector, the `organism` record + derived metabolism, a `population` store with a per-cell occupancy index, the `ecology` tick functions (pure-ish fns over `Space`/`Field`/`Population`), and a `Sim` that owns a `World` + `Population` and composes the per-tick order. Selection stays **implicit**: nothing scores fitness — death and birth are consequences of energy-vs-world.

**Tech Stack:** Rust (edition 2021), std only — no external dependencies (the PRNG is hand-rolled so determinism survives without `rand`).

**Project root:** `~/dev/alife/`. All paths below are relative to it.

**Design source:** `~/dev/ideas/world-design.md` (§C coupling rule, §F working model) and `~/dev/ideas/artificial-life-sim.md` (organism model + language split).

**Builds on (plan 1, already shipped — exact interfaces consumed):**
- `space::{Layer, Coord, Space, Grid2p5D}` — `Coord::new(x,y,layer)`, `Space::{width,height,len,in_bounds,index,planar_neighbors}`.
- `field::Field` — `zeros,len,get,set,add,total,scale_all,as_slice`.
- `world::{Params, World}` — `World{ pub space, pub valaar, pub params, new, add_source, add_access_point, step }`.
- `valaar::{inject_sources, decay, diffuse_planar, exchange_layers}`.

## Global Constraints

- **Language:** Rust, edition 2021. **No external crates** (std only). The PRNG is implemented by hand.
- **Determinism (load-bearing):** identical seed + identical inputs ⇒ identical run, every time. *All* randomness (genome init, mutation, the move dice) flows through one seeded `Rng` threaded explicitly. No `std::collections::HashMap` iteration over organisms, no clock/thread randomness, no float NaN traps. Organisms are always processed in `Vec` index order.
- **Spatial access stays behind `Space`:** organism code calls `Coord`/`Space::{index,planar_neighbors}` — it never assumes a dimensionality or index layout (same rule plan 1 set). Movement uses `planar_neighbors`, so organisms never change `Layer` (cross-layer travel needs a future `digging` gene).
- **Implicit selection only:** no fitness function, no ranking, no "best organism" selection anywhere. Survival is an emergent consequence of the energy economy.
- **Valaar is the one currency:** organism `energy` *is* stored valaar. Absorption removes it from the `Field`; death returns it to the `Field` as detritus. Energy is never created from nothing except at the Rasconne source (plan 1).
- **Every rate is a tunable dial:** all ecology constants live on `EcoParams` (edge-of-chaos tuning is later work, not this plan).
- **No new environmental axes yet:** heat/water fields and the season cycle are a *later* plan, so this plan adds **no** `heat_tolerance`/`drought_tolerance`/`digging` genes — per world-design §C, a trait with no matching axis is just an inert gene. The genome stays minimal and every trait is exercised by the loop.
- **CPU reference core:** still no GPU, no rendering beyond a text census. `Population` is a flat `Vec` so it ports to GPU/`.npy` later.
- **Version control:** `~/dev/alife` is a git repo. Each task ends with a commit. **Commit messages must NOT include a `Co-Authored-By` trailer** (user preference).

**Scope (plan 2 of several).** Later plans: heat & water fields + the 6-arh season cycle (Vraze = crystallisation); mutation-as-a-field; wgpu acceleration + the live renderer; the web viewer; and the Python natural-history/species-clustering + timeline layer (consuming exported snapshots).

---

## Design assumptions (flagged for veto)

These are the engineering choices I made where the design docs were silent. Each is marked `[ASSUMPTION]` at its task too. **Reject/adjust any before execution** — they are deliberately simple v1 picks, not commitments.

- **A1 — 6-trait genome:** `size, valaar_efficiency, speed, diet, repro_threshold, lifespan`, each a single `f32` in `[0,1]`. No brain, no morphology (those are future *clades*). No heat/water/dig genes yet (see constraint above).
- **A2 — diet is one continuous axis:** `0.0` = pure autotroph (absorbs valaar from its cell), `1.0` = pure predator (eats co-located organisms), between = omnivore. Avoids hard trophic classes while still giving producers→herbivores→predators as an emergent spread.
- **A3 — trade-off via `valaar_efficiency`:** high efficiency helps both absorption and predation gain, but `max_energy` (storage/size payoff) comes from `size`, which raises `basal_cost` — so "efficient" and "big" pull against each other through metabolism. (world-design §F asks efficiency to cost max size/output; here the cost is carried by `size`'s metabolism rather than a hard coupling — flag if you want a direct `efficiency↔size` cap instead.)
- **A4 — movement = gradient ascent on valaar,** with move-*chance* = `speed` and move-*cost* = `move_cost·speed`. Predators follow valaar too (prey cluster where valaar is, so it doubles as a prey gradient). No sensing radius gene in v1 (only immediate `planar_neighbors`).
- **A5 — predation resolution:** at most **one** kill per cell per tick — the highest-power attacker (`size·diet`, ties→lowest index) eats the lowest-`size` other occupant, but only if attacker `diet > 0.5` and attacker `size >` victim `size`. Victim energy → 0 (culled next), attacker gains `victim.energy · predation_efficiency · attacker.valaar_efficiency`.
- **A6 — reproduction is asexual, energy-split:** an organism with `energy ≥ repro_threshold · max_energy` spawns one child in its own cell; child takes `repro_cost_fraction` of the parent's energy; child genome = parent's, mutated (each trait `+= U[-rate,rate]`, clamped to `[0,1]`).
- **A7 — death → detritus:** an organism dies when `energy ≤ 0` **or** `age ≥ lifespan`; on death it returns `max(energy,0)·detritus_fraction` valaar to its cell (recycling). Starvation deaths return ~nothing; old-age deaths return a chunk.
- **A8 — tick order:** `world.step()` (substrate) → `absorb` → `move` → `predate` → `metabolize`(+age) → `cull_and_recycle` → `reproduce`. Flag if you want feeding after movement settles differently, or predation before absorption.
- **A9 — default dials** (`EcoParams::default`) are placeholder numbers chosen to be *runnable*, not tuned. Finding the living regime is explicit future work.
- **A10 — naming:** modules/types use plain engineering names (`Organism`, `predate`, …). The *creative* names for trophic tiers / species are yours to coin later (world-design §F leaves them "TBD"); I won't invent lore names.

---

## File Structure

- `src/rng.rs` — `Rng`: a seeded SplitMix64 PRNG. One responsibility: deterministic pseudo-randomness.
- `src/genome.rs` — `Genome`: the trait vector + `random`/`mutate`/`clamped`. Pure data + value transforms.
- `src/params.rs` — `EcoParams`: every ecology dial in one struct (so `Organism` and `ecology` share it without a cycle).
- `src/organism.rs` — `Organism`: genome + position + energy + age, and the derived metabolic quantities.
- `src/population.rs` — `Population`: the organism store + per-cell occupancy index + cull.
- `src/ecology.rs` — the tick functions (`absorb`, `move_organisms`, `predate`, `metabolize`, `cull_and_recycle`, `reproduce`). No state of its own.
- `src/sim.rs` — `Sim`: owns `World` + `Population` + `EcoParams` + `Rng`; `step()` composes the order; seeding + census helpers.
- `src/bin/life.rs` — headless census runner.
- `tests/ecology.rs` — end-to-end "life persists & is deterministic" integration test.

Each `src/lib.rs` edit adds exactly one `pub mod`, shown in its task.

---

### Task 1: Deterministic PRNG (`rng`)

**Files:**
- Create: `src/rng.rs`
- Modify: `src/lib.rs` (add `pub mod rng;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct Rng` with `pub fn new(seed: u64) -> Rng`, `pub fn next_u64(&mut self) -> u64`, `pub fn next_unit(&mut self) -> f32` (in `0.0..1.0`), `pub fn next_range(&mut self, lo: f32, hi: f32) -> f32`.

- [ ] **Step 1: Add the module**

Set `src/lib.rs` to (append the new line):

```rust
pub mod space;
pub mod field;
pub mod world;
pub mod valaar;
pub mod rng;
```

- [ ] **Step 2: Write the failing tests**

Create `src/rng.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        // Extremely unlikely to collide on the first draw for SplitMix64.
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn next_unit_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let u = r.next_unit();
            assert!((0.0..1.0).contains(&u), "out of range: {u}");
        }
    }

    #[test]
    fn next_range_respects_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..1000 {
            let v = r.next_range(-2.0, 5.0);
            assert!((-2.0..5.0).contains(&v), "out of range: {v}");
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib rng`
Expected: **compile error** — `cannot find type Rng`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/rng.rs`:

```rust
//! A tiny seeded PRNG (SplitMix64). Hand-rolled so the whole sim stays
//! deterministic and std-only — identical seed ⇒ identical stream.

/// Deterministic pseudo-random generator. Not cryptographic.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// SplitMix64: advance state, then bit-mix to an output word.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform `f32` in `[0.0, 1.0)` using the top 24 bits (mantissa width).
    pub fn next_unit(&mut self) -> f32 {
        let bits = self.next_u64() >> 40; // keep 24 bits
        bits as f32 / (1u64 << 24) as f32
    }

    /// Uniform `f32` in `[lo, hi)`.
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_unit()
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib rng`
Expected: `test result: ok. 4 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: deterministic SplitMix64 rng"
```

---

### Task 2: The genome (`genome`)

**Files:**
- Create: `src/genome.rs`
- Modify: `src/lib.rs` (add `pub mod genome;`)

**Interfaces:**
- Consumes: `Rng` (Task 1).
- Produces: `pub const TRAIT_COUNT: usize = 6;` and `pub struct Genome { pub size: f32, pub valaar_efficiency: f32, pub speed: f32, pub diet: f32, pub repro_threshold: f32, pub lifespan: f32 }` with `pub fn from_array([f32; TRAIT_COUNT]) -> Genome`, `pub fn to_array(&self) -> [f32; TRAIT_COUNT]`, `pub fn clamped(self) -> Genome`, `pub fn random(rng: &mut Rng) -> Genome`, `pub fn mutate(&self, rng: &mut Rng, rate: f32) -> Genome`. `[ASSUMPTION A1]`

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod genome;
```

- [ ] **Step 2: Write the failing tests**

Create `src/genome.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn round_trips_through_array() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
        assert_eq!(g.to_array(), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
        assert_eq!(g.diet, 0.4);
    }

    #[test]
    fn clamped_pins_to_unit_interval() {
        let g = Genome::from_array([-1.0, 2.0, 0.5, 0.5, 0.5, 0.5]).clamped();
        assert_eq!(g.size, 0.0);
        assert_eq!(g.valaar_efficiency, 1.0);
        assert_eq!(g.speed, 0.5);
    }

    #[test]
    fn random_is_seed_deterministic_and_in_bounds() {
        let a = Genome::random(&mut Rng::new(123));
        let b = Genome::random(&mut Rng::new(123));
        assert_eq!(a, b);
        for t in a.to_array() {
            assert!((0.0..=1.0).contains(&t), "out of range: {t}");
        }
    }

    #[test]
    fn mutate_stays_in_bounds_even_from_extremes() {
        let mut r = Rng::new(5);
        let g = Genome::from_array([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        for _ in 0..200 {
            let m = g.mutate(&mut r, 0.1);
            for t in m.to_array() {
                assert!((0.0..=1.0).contains(&t), "out of range: {t}");
            }
        }
    }

    #[test]
    fn mutate_with_zero_rate_is_identity() {
        let mut r = Rng::new(5);
        let g = Genome::from_array([0.2, 0.3, 0.4, 0.5, 0.6, 0.7]);
        assert_eq!(g.mutate(&mut r, 0.0), g);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib genome`
Expected: **compile error** — `cannot find type Genome`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/genome.rs`:

```rust
//! The trait-vector genome. Every field is a scalar in `[0, 1]`; behaviour and
//! metabolism read these directly, and reproduction perturbs them. Designed so a
//! brain/morphology can be added later as a *new clade*, not a rewrite.

use crate::rng::Rng;

/// Number of scalar traits in the genome.
pub const TRAIT_COUNT: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Genome {
    /// Body size: more storage and predation power, but costlier metabolism.
    pub size: f32,
    /// Fraction of contacted valaar actually captured (absorption & predation).
    pub valaar_efficiency: f32,
    /// Movement tendency and per-move cost.
    pub speed: f32,
    /// Trophic axis: 0 = autotroph, 1 = predator.
    pub diet: f32,
    /// Reproduce once energy reaches this fraction of max storage.
    pub repro_threshold: f32,
    /// Maps (via EcoParams) to a maximum age in ticks.
    pub lifespan: f32,
}

impl Genome {
    pub fn from_array(a: [f32; TRAIT_COUNT]) -> Self {
        Genome {
            size: a[0],
            valaar_efficiency: a[1],
            speed: a[2],
            diet: a[3],
            repro_threshold: a[4],
            lifespan: a[5],
        }
    }

    pub fn to_array(&self) -> [f32; TRAIT_COUNT] {
        [
            self.size,
            self.valaar_efficiency,
            self.speed,
            self.diet,
            self.repro_threshold,
            self.lifespan,
        ]
    }

    /// Pin every trait into `[0, 1]`.
    pub fn clamped(self) -> Self {
        let mut a = self.to_array();
        for t in &mut a {
            *t = t.clamp(0.0, 1.0);
        }
        Genome::from_array(a)
    }

    /// A fresh genome with every trait drawn uniformly from `[0, 1)`.
    pub fn random(rng: &mut Rng) -> Self {
        let mut a = [0.0f32; TRAIT_COUNT];
        for t in &mut a {
            *t = rng.next_unit();
        }
        Genome::from_array(a)
    }

    /// A mutated copy: each trait nudged by `U[-rate, rate]`, then clamped.
    pub fn mutate(&self, rng: &mut Rng, rate: f32) -> Self {
        let mut a = self.to_array();
        for t in &mut a {
            *t += rng.next_range(-rate, rate);
        }
        Genome::from_array(a).clamped()
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib genome`
Expected: `test result: ok. 5 passed`.

(`mutate_with_zero_rate_is_identity` passes because `next_range(0,0)` returns `0.0` and still advances the rng — the identity holds on the value.)

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: trait-vector genome with mutation"
```

---

### Task 3: Ecology params + organism (`params`, `organism`)

**Files:**
- Create: `src/params.rs`
- Create: `src/organism.rs`
- Modify: `src/lib.rs` (add `pub mod params;` and `pub mod organism;`)

**Interfaces:**
- Consumes: `Genome` (Task 2); `Coord` (plan 1).
- Produces:
  - `pub struct EcoParams { pub base_energy: f32, pub size_energy: f32, pub basal_cost: f32, pub size_cost: f32, pub uptake_rate: f32, pub move_cost: f32, pub detritus_fraction: f32, pub repro_cost_fraction: f32, pub mutation_rate: f32, pub min_lifespan: u32, pub max_lifespan: u32, pub predation_efficiency: f32, pub initial_energy: f32 }` + `impl Default`. `[ASSUMPTION A9]`
  - `pub struct Organism { pub genome: Genome, pub pos: Coord, pub energy: f32, pub age: u32 }` with `pub fn new(genome: Genome, pos: Coord, energy: f32) -> Organism`, `pub fn max_energy(&self, eco: &EcoParams) -> f32`, `pub fn basal_cost(&self, eco: &EcoParams) -> f32`, `pub fn lifespan_ticks(&self, eco: &EcoParams) -> u32`, `pub fn is_alive(&self, eco: &EcoParams) -> bool`. `[ASSUMPTION A3, A7]`

- [ ] **Step 1: Add the modules**

Append to `src/lib.rs`:

```rust
pub mod params;
pub mod organism;
```

- [ ] **Step 2: Write the failing tests**

Create `src/params.rs`:

```rust
//! Every ecology rate in one struct so `organism` and `ecology` share it
//! without a module cycle. All values are dials (edge-of-chaos tuning is later).

/// Tunable rates for the ecology loop. Defaults are *runnable placeholders*,
/// not a tuned living regime.
#[derive(Clone, Copy, Debug)]
pub struct EcoParams {
    /// Max energy storage of a size-0 organism.
    pub base_energy: f32,
    /// Extra max storage per unit `size`.
    pub size_energy: f32,
    /// Baseline energy spent per tick by a size-0 organism.
    pub basal_cost: f32,
    /// Extra basal cost per unit `size`.
    pub size_cost: f32,
    /// Fraction of a cell's valaar an ideal autotroph captures per tick.
    pub uptake_rate: f32,
    /// Energy spent to move one cell (scaled by `speed`).
    pub move_cost: f32,
    /// Fraction of a dead organism's energy returned to its cell.
    pub detritus_fraction: f32,
    /// Fraction of parent energy handed to a new child.
    pub repro_cost_fraction: f32,
    /// Per-trait mutation magnitude at birth.
    pub mutation_rate: f32,
    /// Minimum lifespan in ticks (lifespan gene 0.0).
    pub min_lifespan: u32,
    /// Maximum lifespan in ticks (lifespan gene 1.0).
    pub max_lifespan: u32,
    /// Fraction of prey energy a predator captures.
    pub predation_efficiency: f32,
    /// Energy a seeded/initial organism starts with.
    pub initial_energy: f32,
}

impl Default for EcoParams {
    fn default() -> Self {
        EcoParams {
            base_energy: 1.0,
            size_energy: 4.0,
            basal_cost: 0.02,
            size_cost: 0.05,
            uptake_rate: 0.5,
            move_cost: 0.05,
            detritus_fraction: 0.5,
            repro_cost_fraction: 0.4,
            mutation_rate: 0.05,
            min_lifespan: 20,
            max_lifespan: 200,
            predation_efficiency: 0.6,
            initial_energy: 2.0,
        }
    }
}
```

Create `src/organism.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::params::EcoParams;
    use crate::space::{Coord, Layer};

    fn genome_with(size: f32, lifespan: f32) -> Genome {
        Genome::from_array([size, 0.5, 0.5, 0.5, 0.5, lifespan])
    }

    #[test]
    fn bigger_body_stores_and_costs_more() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let small = Organism::new(genome_with(0.0, 0.5), pos, 1.0);
        let big = Organism::new(genome_with(1.0, 0.5), pos, 1.0);
        assert!(big.max_energy(&eco) > small.max_energy(&eco));
        assert!(big.basal_cost(&eco) > small.basal_cost(&eco));
    }

    #[test]
    fn lifespan_ticks_span_the_configured_range() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let young = Organism::new(genome_with(0.5, 0.0), pos, 1.0);
        let old = Organism::new(genome_with(0.5, 1.0), pos, 1.0);
        assert_eq!(young.lifespan_ticks(&eco), eco.min_lifespan);
        assert_eq!(old.lifespan_ticks(&eco), eco.max_lifespan);
    }

    #[test]
    fn is_alive_tracks_energy_and_age() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let mut o = Organism::new(genome_with(0.5, 0.0), pos, 1.0);
        assert!(o.is_alive(&eco)); // young, fed
        o.energy = 0.0;
        assert!(!o.is_alive(&eco)); // starved
        o.energy = 1.0;
        o.age = o.lifespan_ticks(&eco);
        assert!(!o.is_alive(&eco)); // too old
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib organism`
Expected: **compile error** — `cannot find type Organism`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/organism.rs`:

```rust
//! A single organism: its genome, where it is, how much valaar it has stored
//! (`energy`), and how old it is. Metabolic quantities are derived from the
//! genome through `EcoParams` so the genome stays pure data.

use crate::genome::Genome;
use crate::params::EcoParams;
use crate::space::Coord;

#[derive(Clone, Debug)]
pub struct Organism {
    pub genome: Genome,
    pub pos: Coord,
    /// Stored valaar. This *is* the energy currency.
    pub energy: f32,
    pub age: u32,
}

impl Organism {
    pub fn new(genome: Genome, pos: Coord, energy: f32) -> Self {
        Organism { genome, pos, energy, age: 0 }
    }

    /// Maximum energy this body can store.
    pub fn max_energy(&self, eco: &EcoParams) -> f32 {
        eco.base_energy + self.genome.size * eco.size_energy
    }

    /// Energy spent just staying alive this tick.
    pub fn basal_cost(&self, eco: &EcoParams) -> f32 {
        eco.basal_cost + self.genome.size * eco.size_cost
    }

    /// Maximum age in ticks (genome `lifespan` lerps min..max).
    pub fn lifespan_ticks(&self, eco: &EcoParams) -> u32 {
        let span = (eco.max_lifespan - eco.min_lifespan) as f32;
        eco.min_lifespan + (self.genome.lifespan * span) as u32
    }

    /// Alive iff it still has energy and has not outlived its lifespan.
    pub fn is_alive(&self, eco: &EcoParams) -> bool {
        self.energy > 0.0 && self.age < self.lifespan_ticks(eco)
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib organism`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: EcoParams + Organism with derived metabolism"
```

---

### Task 4: Population store + occupancy (`population`)

**Files:**
- Create: `src/population.rs`
- Modify: `src/lib.rs` (add `pub mod population;`)

**Interfaces:**
- Consumes: `Organism` (Task 3); `Space`, `Coord` (plan 1).
- Produces: `pub struct Population` with `pub fn new() -> Population`, `pub fn len(&self) -> usize`, `pub fn is_empty(&self) -> bool`, `pub fn spawn(&mut self, o: Organism)`, `pub fn organisms(&self) -> &[Organism]`, `pub fn organisms_mut(&mut self) -> &mut [Organism]`, `pub fn occupancy<S: Space>(&self, space: &S) -> Vec<Vec<usize>>`, `pub fn retain(&mut self, keep: impl Fn(&Organism) -> bool)`. Plus `impl Default`.

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod population;
```

- [ ] **Step 2: Write the failing tests**

Create `src/population.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::organism::Organism;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    fn org_at(c: Coord) -> Organism {
        Organism::new(Genome::from_array([0.5; 6]), c, 1.0)
    }

    #[test]
    fn spawn_grows_population() {
        let mut p = Population::new();
        assert!(p.is_empty());
        p.spawn(org_at(Coord::new(0, 0, Layer::Surface)));
        p.spawn(org_at(Coord::new(1, 0, Layer::Surface)));
        assert_eq!(p.len(), 2);
        assert!(!p.is_empty());
    }

    #[test]
    fn occupancy_buckets_by_cell() {
        let space = Grid2p5D::new(4, 4);
        let mut p = Population::new();
        let c = Coord::new(2, 2, Layer::Surface);
        p.spawn(org_at(c));
        p.spawn(org_at(c)); // same cell
        p.spawn(org_at(Coord::new(0, 0, Layer::Surface)));
        let occ = p.occupancy(&space);
        assert_eq!(occ.len(), space.len());
        assert_eq!(occ[space.index(c)].len(), 2);
        assert_eq!(occ[space.index(Coord::new(0, 0, Layer::Surface))].len(), 1);
    }

    #[test]
    fn retain_drops_unwanted() {
        let mut p = Population::new();
        let mut a = org_at(Coord::new(0, 0, Layer::Surface));
        a.energy = 0.0;
        p.spawn(a);
        p.spawn(org_at(Coord::new(1, 0, Layer::Surface))); // energy 1.0
        p.retain(|o| o.energy > 0.0);
        assert_eq!(p.len(), 1);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib population`
Expected: **compile error** — `cannot find type Population`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/population.rs`:

```rust
//! The organism store: a flat `Vec<Organism>` (GPU/.npy-portable later) plus a
//! per-cell occupancy index rebuilt on demand for local interactions.

use crate::organism::Organism;
use crate::space::Space;

#[derive(Default)]
pub struct Population {
    orgs: Vec<Organism>,
}

impl Population {
    pub fn new() -> Self {
        Population { orgs: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.orgs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orgs.is_empty()
    }

    pub fn spawn(&mut self, o: Organism) {
        self.orgs.push(o);
    }

    pub fn organisms(&self) -> &[Organism] {
        &self.orgs
    }

    pub fn organisms_mut(&mut self) -> &mut [Organism] {
        &mut self.orgs
    }

    /// Lists of organism indices per cell, indexed by `Space::index`. Rebuilt
    /// each call so it always matches the current positions.
    pub fn occupancy<S: Space>(&self, space: &S) -> Vec<Vec<usize>> {
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); space.len()];
        for (i, o) in self.orgs.iter().enumerate() {
            cells[space.index(o.pos)].push(i);
        }
        cells
    }

    /// Keep only organisms for which `keep` is true (preserves order).
    pub fn retain(&mut self, keep: impl Fn(&Organism) -> bool) {
        self.orgs.retain(|o| keep(o));
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib population`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: Population store + per-cell occupancy"
```

---

### Task 5: Autotrophy — absorb valaar (`ecology::absorb`)

**Files:**
- Create: `src/ecology.rs`
- Modify: `src/lib.rs` (add `pub mod ecology;`)

**Interfaces:**
- Consumes: `Space` (plan 1), `Field` (plan 1), `Population` (Task 4), `EcoParams` (Task 3).
- Produces: `pub fn absorb<S: Space>(space: &S, field: &mut Field, pop: &mut Population, eco: &EcoParams)`. `[ASSUMPTION A2, A3]`

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod ecology;
```

- [ ] **Step 2: Write the failing tests**

Create `src/ecology.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::organism::Organism;
    use crate::params::EcoParams;
    use crate::population::Population;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    // [size, valaar_efficiency, speed, diet, repro_threshold, lifespan]
    fn genome(diet: f32, eff: f32) -> Genome {
        Genome::from_array([0.5, eff, 0.0, diet, 0.9, 0.5])
    }

    #[test]
    fn autotroph_absorbs_and_conserves() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(1, 1, Layer::Surface);
        field.set(space.index(c), 10.0);
        let mut pop = Population::new();
        pop.spawn(Organism::new(genome(0.0, 1.0), c, 1.0)); // pure autotroph

        let field_before = field.total();
        let energy_before = pop.organisms()[0].energy;
        absorb(&space, &mut field, &mut pop, &eco);
        let gained = pop.organisms()[0].energy - energy_before;
        let lost = field_before - field.total();

        assert!(gained > 0.0, "autotroph should gain energy");
        assert!((gained - lost).abs() < 1e-5, "valaar must be conserved");
    }

    #[test]
    fn pure_predator_absorbs_nothing() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(1, 1, Layer::Surface);
        field.set(space.index(c), 10.0);
        let mut pop = Population::new();
        pop.spawn(Organism::new(genome(1.0, 1.0), c, 1.0)); // pure predator

        absorb(&space, &mut field, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 1.0);
        assert_eq!(field.total(), 10.0);
    }

    #[test]
    fn absorption_is_capped_by_storage() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(1, 1, Layer::Surface);
        field.set(space.index(c), 1000.0);
        let mut pop = Population::new();
        let o = Organism::new(genome(0.0, 1.0), c, 0.0);
        let cap = o.max_energy(&eco);
        pop.spawn(o);

        absorb(&space, &mut field, &mut pop, &eco);
        assert!(pop.organisms()[0].energy <= cap + 1e-5, "must not exceed storage");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `cannot find function absorb`.

- [ ] **Step 4: Write the minimal implementation**

Insert above the `tests` module in `src/ecology.rs`:

```rust
//! The ecology loop as a set of pure-ish tick functions over the substrate
//! (`Space`/`Field`) and the `Population`. No hidden state; ordering lives in
//! `Sim::step`. Selection is implicit — nothing here scores fitness.

use crate::field::Field;
use crate::params::EcoParams;
use crate::population::Population;
use crate::space::Space;

/// Autotrophy: each organism with an autotroph fraction `(1 - diet)` draws
/// valaar from the cell it stands in, scaled by `valaar_efficiency`, capped by
/// what's present and by remaining storage. The drawn valaar leaves the field.
pub fn absorb<S: Space>(space: &S, field: &mut Field, pop: &mut Population, eco: &EcoParams) {
    for o in pop.organisms_mut() {
        let auto = 1.0 - o.genome.diet;
        if auto <= 0.0 {
            continue;
        }
        let i = space.index(o.pos);
        let avail = field.get(i);
        if avail <= 0.0 {
            continue;
        }
        let room = (o.max_energy(eco) - o.energy).max(0.0);
        let want = eco.uptake_rate * o.genome.valaar_efficiency * auto * avail;
        let gain = want.min(avail).min(room);
        field.add(i, -gain);
        o.energy += gain;
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: ecology::absorb (autotrophy from the valaar field)"
```

---

### Task 6: Metabolism + death → detritus (`ecology::metabolize`, `ecology::cull_and_recycle`)

**Files:**
- Modify: `src/ecology.rs` (add two fns; extend `tests`)

**Interfaces:**
- Consumes: `Space`, `Field`, `Population`, `EcoParams`.
- Produces: `pub fn metabolize(pop: &mut Population, eco: &EcoParams)`, `pub fn cull_and_recycle<S: Space>(space: &S, field: &mut Field, pop: &mut Population, eco: &EcoParams)`. `[ASSUMPTION A7]`

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn metabolize_spends_energy_and_ages() {
        let eco = EcoParams::default();
        let c = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        let o = Organism::new(genome(0.0, 1.0), c, 5.0);
        let cost = o.basal_cost(&eco);
        pop.spawn(o);
        metabolize(&mut pop, &eco);
        assert!((pop.organisms()[0].energy - (5.0 - cost)).abs() < 1e-6);
        assert_eq!(pop.organisms()[0].age, 1);
    }

    #[test]
    fn starved_organism_is_culled() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        let mut o = Organism::new(genome(0.0, 1.0), c, 0.0); // already starved
        o.energy = 0.0;
        pop.spawn(o);
        cull_and_recycle(&space, &mut field, &mut pop, &eco);
        assert_eq!(pop.len(), 0);
    }

    #[test]
    fn old_age_death_returns_detritus() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        let mut o = Organism::new(genome(0.0, 1.0), c, 4.0);
        o.age = o.lifespan_ticks(&eco); // too old, but still has energy
        let expected = 4.0 * eco.detritus_fraction;
        pop.spawn(o);
        cull_and_recycle(&space, &mut field, &mut pop, &eco);
        assert_eq!(pop.len(), 0);
        assert!((field.get(space.index(c)) - expected).abs() < 1e-6);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `cannot find function metabolize` / `cull_and_recycle`.

- [ ] **Step 3: Write the minimal implementation**

Add to `src/ecology.rs` (below `absorb`):

```rust
/// Spend basal energy, cap storage, and age every organism by one tick.
pub fn metabolize(pop: &mut Population, eco: &EcoParams) {
    for o in pop.organisms_mut() {
        o.energy -= o.basal_cost(eco);
        let cap = o.max_energy(eco);
        if o.energy > cap {
            o.energy = cap;
        }
        o.age += 1;
    }
}

/// Return each dead organism's remaining energy to its cell as detritus
/// (recycling), then drop the dead from the population.
pub fn cull_and_recycle<S: Space>(
    space: &S,
    field: &mut Field,
    pop: &mut Population,
    eco: &EcoParams,
) {
    for o in pop.organisms() {
        if !o.is_alive(eco) {
            let detritus = o.energy.max(0.0) * eco.detritus_fraction;
            if detritus > 0.0 {
                field.add(space.index(o.pos), detritus);
            }
        }
    }
    pop.retain(|o| o.is_alive(eco));
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: `test result: ok. 6 passed`.

- [ ] **Step 5: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: ecology metabolism + death-to-detritus recycling"
```

---

### Task 7: Movement — gradient ascent (`ecology::move_organisms`)

**Files:**
- Modify: `src/ecology.rs` (add fn; extend `tests`)

**Interfaces:**
- Consumes: `Space`, `Field`, `Population`, `EcoParams`, `Rng` (Task 1).
- Produces: `pub fn move_organisms<S: Space>(space: &S, field: &Field, pop: &mut Population, eco: &EcoParams, rng: &mut Rng)`. `[ASSUMPTION A4]`

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/ecology.rs` (add `use crate::rng::Rng;` to the test `use`s if not present):

```rust
    #[test]
    fn moves_uphill_toward_richer_valaar() {
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        // Increasing valaar to the right.
        for x in 0..4u32 {
            field.set(space.index(Coord::new(x, 0, Layer::Surface)), x as f32);
        }
        let start = Coord::new(1, 0, Layer::Surface);
        let mut pop = Population::new();
        // speed 1.0 => always moves.
        pop.spawn(Organism::new(Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5]), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng);
        assert_eq!(pop.organisms()[0].pos, Coord::new(2, 0, Layer::Surface));
        assert!(pop.organisms()[0].energy < 5.0, "moving costs energy");
    }

    #[test]
    fn at_local_max_it_stays_put() {
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let peak = Coord::new(2, 0, Layer::Surface);
        field.set(space.index(peak), 100.0);
        let mut pop = Population::new();
        pop.spawn(Organism::new(Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5]), peak, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng);
        assert_eq!(pop.organisms()[0].pos, peak);
        assert_eq!(pop.organisms()[0].energy, 5.0, "no move, no cost");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `cannot find function move_organisms`.

- [ ] **Step 3: Write the minimal implementation**

Add `use crate::rng::Rng;` to the top-of-file `use`s in `src/ecology.rs`, then add (below `cull_and_recycle`):

```rust
/// Each organism moves with probability `speed` toward its richest in-bounds
/// planar neighbour (gradient ascent on valaar). Moving costs `move_cost·speed`.
/// Neighbours never cross layers, so organisms stay on their layer.
pub fn move_organisms<S: Space>(
    space: &S,
    field: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
) {
    for o in pop.organisms_mut() {
        // Draw first so the rng stream advances once per organism regardless.
        if rng.next_unit() >= o.genome.speed {
            continue;
        }
        let mut best = o.pos;
        let mut best_v = field.get(space.index(o.pos));
        for n in space.planar_neighbors(o.pos) {
            let v = field.get(space.index(n));
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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: `test result: ok. 8 passed`.

- [ ] **Step 5: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: ecology movement (valaar gradient ascent)"
```

---

### Task 8: Predation (`ecology::predate`)

**Files:**
- Modify: `src/ecology.rs` (add fn; extend `tests`)

**Interfaces:**
- Consumes: `Space`, `Population`, `EcoParams`.
- Produces: `pub fn predate<S: Space>(space: &S, pop: &mut Population, eco: &EcoParams)`. `[ASSUMPTION A5]`

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/ecology.rs`:

```rust
    // [size, eff, speed, diet, repro_threshold, lifespan]
    fn predator(size: f32) -> Genome {
        Genome::from_array([size, 1.0, 0.0, 1.0, 0.9, 0.5])
    }
    fn prey(size: f32) -> Genome {
        Genome::from_array([size, 1.0, 0.0, 0.0, 0.9, 0.5])
    }

    #[test]
    fn predator_eats_smaller_co_located_prey() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(Organism::new(predator(0.9), c, 1.0)); // big predator
        pop.spawn(Organism::new(prey(0.2), c, 3.0)); // small prey, energy 3

        predate(&space, &mut pop, &eco);

        let pred = &pop.organisms()[0];
        let victim = &pop.organisms()[1];
        assert!(pred.energy > 1.0, "predator should gain");
        assert_eq!(victim.energy, 0.0, "prey should be drained");
    }

    #[test]
    fn lone_organism_is_not_eaten() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(Organism::new(predator(0.9), c, 1.0));
        predate(&space, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 1.0);
    }

    #[test]
    fn autotrophs_do_not_predate() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(Organism::new(prey(0.9), c, 1.0)); // big but diet 0
        pop.spawn(Organism::new(prey(0.2), c, 3.0));
        predate(&space, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 1.0);
        assert_eq!(pop.organisms()[1].energy, 3.0);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `cannot find function predate`.

- [ ] **Step 3: Write the minimal implementation**

Add to `src/ecology.rs` (below `move_organisms`):

```rust
/// Resolve at most one predation per cell: the strongest predator
/// (`size·diet`, ties→lowest index) eats the smallest other occupant, but only
/// if it is a real predator (`diet > 0.5`) and strictly bigger than its victim.
/// Prey energy is drained to zero (it dies next cull); the predator banks
/// `prey.energy · predation_efficiency · valaar_efficiency`, capped by storage.
pub fn predate<S: Space>(space: &S, pop: &mut Population, eco: &EcoParams) {
    let occ = pop.occupancy(space);
    let orgs = pop.organisms_mut();
    for cell in &occ {
        if cell.len() < 2 {
            continue;
        }
        // Strongest attacker by power = size * diet (ties → lowest index).
        let mut attacker = cell[0];
        for &i in cell {
            let pi = orgs[i].genome.size * orgs[i].genome.diet;
            let pa = orgs[attacker].genome.size * orgs[attacker].genome.diet;
            if pi > pa {
                attacker = i;
            }
        }
        if orgs[attacker].genome.diet <= 0.5 {
            continue; // no real predator here
        }
        // Smallest victim among the others (ties → lowest index).
        let mut victim: Option<usize> = None;
        for &i in cell {
            if i == attacker {
                continue;
            }
            match victim {
                None => victim = Some(i),
                Some(v) if orgs[i].genome.size < orgs[v].genome.size => victim = Some(i),
                _ => {}
            }
        }
        let victim = match victim {
            Some(v) => v,
            None => continue,
        };
        if orgs[attacker].genome.size <= orgs[victim].genome.size {
            continue; // can't overpower
        }
        let prey_energy = orgs[victim].energy;
        let gain = prey_energy * eco.predation_efficiency * orgs[attacker].genome.valaar_efficiency;
        orgs[victim].energy = 0.0;
        let cap = orgs[attacker].max_energy(eco);
        orgs[attacker].energy = (orgs[attacker].energy + gain).min(cap);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: `test result: ok. 11 passed`.

- [ ] **Step 5: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: ecology predation (one kill per cell)"
```

---

### Task 9: Reproduction + mutation (`ecology::reproduce`)

**Files:**
- Modify: `src/ecology.rs` (add fn; extend `tests`)

**Interfaces:**
- Consumes: `Population`, `EcoParams`, `Rng`, `Organism`, `Genome`.
- Produces: `pub fn reproduce(pop: &mut Population, eco: &EcoParams, rng: &mut Rng)`. `[ASSUMPTION A6]`

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn well_fed_organism_spawns_one_child() {
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut pop = Population::new();
        // repro_threshold 0.0 => any positive energy triggers reproduction.
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.0, 0.5]);
        let parent = Organism::new(g, c, 5.0);
        let max_e = parent.max_energy(&eco);
        pop.spawn(parent);
        let mut rng = Rng::new(3);
        reproduce(&mut pop, &eco, &mut rng);
        assert_eq!(pop.len(), 2);
        let child = &pop.organisms()[1];
        assert_eq!(child.pos, c);
        assert!((child.energy - 5.0 * eco.repro_cost_fraction).abs() < 1e-6);
        // parent paid for it
        assert!(pop.organisms()[0].energy < 5.0);
        let _ = max_e;
    }

    #[test]
    fn starving_organism_does_not_reproduce() {
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut pop = Population::new();
        // repro_threshold 1.0 => needs full storage; give it almost none.
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 1.0, 0.5]);
        pop.spawn(Organism::new(g, c, 0.1));
        let mut rng = Rng::new(3);
        reproduce(&mut pop, &eco, &mut rng);
        assert_eq!(pop.len(), 1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `cannot find function reproduce`.

- [ ] **Step 3: Write the minimal implementation**

Add `use crate::organism::Organism;` to the top-of-file `use`s in `src/ecology.rs`, then add (below `predate`):

```rust
/// Asexual reproduction: any organism at or above its energy threshold spawns
/// one child in its own cell. The child takes `repro_cost_fraction` of the
/// parent's energy and a mutated copy of its genome. Children are collected
/// first, then appended, so iteration order (and determinism) is stable.
pub fn reproduce(pop: &mut Population, eco: &EcoParams, rng: &mut Rng) {
    let mut children: Vec<Organism> = Vec::new();
    for o in pop.organisms_mut() {
        let threshold = o.genome.repro_threshold * o.max_energy(eco);
        if o.energy >= threshold && o.energy > 0.0 {
            let child_energy = o.energy * eco.repro_cost_fraction;
            o.energy -= child_energy;
            let child_genome = o.genome.mutate(rng, eco.mutation_rate);
            children.push(Organism::new(child_genome, o.pos, child_energy));
        }
    }
    for c in children {
        pop.spawn(c);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: `test result: ok. 13 passed`.

- [ ] **Step 5: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: ecology reproduction with mutation"
```

---

### Task 10: `Sim` orchestration + census runner + integration test

**Files:**
- Create: `src/sim.rs`
- Modify: `src/lib.rs` (add `pub mod sim;`)
- Create: `src/bin/life.rs`
- Create: `tests/ecology.rs`

**Interfaces:**
- Consumes: `World`, `Params` (plan 1); `Population` (Task 4); `EcoParams` (Task 3); `Rng` (Task 1); `ecology::*` (Tasks 5–9); `Grid2p5D`, `Coord`, `Layer`, `Space` (plan 1); `Genome` (Task 2); `Organism` (Task 3).
- Produces: `pub struct Sim<S: Space> { pub world: World<S>, pub pop: Population, pub eco: EcoParams, pub rng: Rng }` with `pub fn new(world: World<S>, eco: EcoParams, seed: u64) -> Sim<S>`, `pub fn population(&self) -> usize`, `pub fn seed_organism(&mut self, o: Organism)`, `pub fn step(&mut self)`. `[ASSUMPTION A8]`

- [ ] **Step 1: Add the module**

Append to `src/lib.rs`:

```rust
pub mod sim;
```

- [ ] **Step 2: Write the failing integration test**

Create `tests/ecology.rs`:

```rust
use alife::genome::Genome;
use alife::organism::Organism;
use alife::params::EcoParams;
use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::sim::Sim;
use alife::world::{Params, World};

fn seeded_sim(seed: u64) -> Sim<Grid2p5D> {
    let (w, h) = (24u32, 24u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));

    let eco = EcoParams::default();
    let mut sim = Sim::new(world, eco, seed);

    // Warm the valaar field so there is food to seed life into.
    for _ in 0..100 {
        sim.world.step();
    }
    // Seed autotrophs in a patch around the source.
    let mut rng = alife::rng::Rng::new(seed ^ 0xABCD);
    for _ in 0..40 {
        let gx = (w / 2).saturating_sub(3) + (rng.next_unit() * 6.0) as u32;
        let gy = (h / 2).saturating_sub(3) + (rng.next_unit() * 6.0) as u32;
        let pos = Coord::new(gx.min(w - 1), gy.min(h - 1), Layer::Surface);
        // Low diet (autotroph), moderate everything else.
        let g = Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6]);
        sim.seed_organism(Organism::new(g, pos, sim.eco.initial_energy));
    }
    sim
}

#[test]
fn life_persists_under_the_gradient() {
    let mut sim = seeded_sim(7);
    assert_eq!(sim.population(), 40);
    for _ in 0..150 {
        sim.step();
    }
    assert!(sim.population() > 0, "the population went extinct");
}

#[test]
fn runs_are_deterministic() {
    let mut a = seeded_sim(11);
    let mut b = seeded_sim(11);
    for _ in 0..150 {
        a.step();
        b.step();
    }
    assert_eq!(a.population(), b.population(), "same seed must give same population");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd ~/dev/alife && cargo test --test ecology`
Expected: **compile error** — `cannot find type Sim` / `no function seed_organism`.

- [ ] **Step 4: Write the minimal implementation**

Insert into `src/sim.rs`:

```rust
//! `Sim` ties the substrate (`World`) to life (`Population`) and runs the tick
//! in a fixed order. It owns the single `Rng` so the whole simulation is
//! reproducible from `(seed, params, seeds)`.

use crate::ecology;
use crate::organism::Organism;
use crate::params::EcoParams;
use crate::population::Population;
use crate::rng::Rng;
use crate::space::Space;
use crate::world::World;

pub struct Sim<S: Space> {
    pub world: World<S>,
    pub pop: Population,
    pub eco: EcoParams,
    pub rng: Rng,
}

impl<S: Space> Sim<S> {
    pub fn new(world: World<S>, eco: EcoParams, seed: u64) -> Self {
        Sim {
            world,
            pop: Population::new(),
            eco,
            rng: Rng::new(seed),
        }
    }

    /// Current number of living organisms.
    pub fn population(&self) -> usize {
        self.pop.len()
    }

    /// Place an organism into the world (used for the initial seeding).
    pub fn seed_organism(&mut self, o: Organism) {
        self.pop.spawn(o);
    }

    /// Advance one tick: substrate, then the ecology loop in fixed order.
    pub fn step(&mut self) {
        self.world.step();
        ecology::absorb(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
        );
        ecology::predate(&self.world.space, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
        ecology::cull_and_recycle(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::reproduce(&mut self.pop, &self.eco, &mut self.rng);
    }
}
```

- [ ] **Step 5: Run the integration test to verify it passes**

Run: `cd ~/dev/alife && cargo test --test ecology`
Expected: `test result: ok. 2 passed`.

If `life_persists_under_the_gradient` fails (extinction) or the population explodes unbounded, that is an `EcoParams` *tuning* signal, not a code bug — adjust the dials in `EcoParams::default` (Task 3) and note it; the regime hunt is expected future work (`[ASSUMPTION A9]`).

- [ ] **Step 6: Add the census runner**

Create `src/bin/life.rs`:

```rust
//! Headless life census: warm the field, seed autotrophs, run, and print the
//! population and mean genome every so often. No graphics (a later plan).

use alife::genome::Genome;
use alife::organism::Organism;
use alife::params::EcoParams;
use alife::rng::Rng;
use alife::sim::Sim;
use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::world::{Params, World};

fn main() {
    let (w, h) = (48u32, 48u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));
    world.add_access_point(w / 2, h / 2);

    let mut sim = Sim::new(world, EcoParams::default(), 1234);
    for _ in 0..150 {
        sim.world.step();
    }

    let mut rng = Rng::new(99);
    for _ in 0..120 {
        let x = (rng.next_unit() * w as f32) as u32;
        let y = (rng.next_unit() * h as f32) as u32;
        let pos = Coord::new(x.min(w - 1), y.min(h - 1), Layer::Surface);
        let g = Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6]);
        sim.seed_organism(Organism::new(g, pos, sim.eco.initial_energy));
    }

    println!("tick   pop   mean_size mean_diet mean_eff");
    for tick in 0..=600 {
        if tick % 50 == 0 {
            let n = sim.pop.len().max(1) as f32;
            let mut s = 0.0;
            let mut d = 0.0;
            let mut e = 0.0;
            for o in sim.pop.organisms() {
                s += o.genome.size;
                d += o.genome.diet;
                e += o.genome.valaar_efficiency;
            }
            println!(
                "{tick:>4}  {:>4}   {:>8.3} {:>8.3} {:>8.3}",
                sim.pop.len(),
                s / n,
                d / n,
                e / n,
            );
        }
        sim.step();
    }
}
```

- [ ] **Step 7: Run the whole suite and the runner**

Run: `cd ~/dev/alife && cargo test`
Expected: all green — plan-1 tests (18 + 1 gradient), the new `rng`/`genome`/`organism`/`population`/`ecology` unit tests, and `tests/ecology.rs` (2 passed).

Run: `cd ~/dev/alife && cargo run --bin life`
Expected: a census table; `pop` stays `> 0` across the run and the mean genome drifts over ticks (evolution under the gradient). If it goes extinct or explodes, tune `EcoParams::default` (see Step 5 note).

- [ ] **Step 8: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: Sim orchestration + life census runner + ecology integration test"
```

---

## What this delivers

A living world: trait-vector organisms that feed from the valaar gradient the substrate produces, move toward it, prey on each other, age, die back into the field, and reproduce with mutation — an unguided population under purely implicit selection, fully deterministic from a seed, all std-only on the CPU. The genome is a flat vector (ready to export to Parquet/`.npy` for the Python lore layer) and the loop is split into pure functions (ready to move onto wgpu later).

**Next plans (not in scope here):** heat + water fields and the 6-arh season cycle (Vraze = crystallisation, the underground `digging` gene, the matching tolerance genes); mutation-as-a-field; wgpu acceleration + the live renderer; the web viewer; and the Python natural-history layer (species clustering + biome maps + the timeline) reading exported snapshots.

## Self-review notes

- **Spec coverage:** organism/genome model (A1, Tasks 2–3); energy economy absorb→spend→die→recycle (Tasks 5–6); movement/niche-seeking (Task 7); trophic predation (Task 8); reproduction+mutation/turnover (Task 9); implicit selection + determinism (global constraints, Task 10). Heat/water/seasons and the lore layer are explicitly deferred per world-design's staging.
- **Type consistency:** genome trait order `[size, valaar_efficiency, speed, diet, repro_threshold, lifespan]` is identical in `from_array`/`to_array` and every test fixture; `EcoParams` field names are used verbatim in `organism`, `ecology`, and tests; `Sim` fields (`world`, `pop`, `eco`, `rng`) match their uses in `step` and the integration test.
- **Determinism:** the only randomness is `Rng` (genome init, mutation, the per-organism move draw); organisms are always processed in `Vec` order; occupancy is built from positions, not hashed iteration.
