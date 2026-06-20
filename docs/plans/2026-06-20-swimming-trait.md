# Swimming Trait (Valaar-crossing + Lamarckian disuse) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a heritable **swimming** trait: organisms whose `swim` gene clears a threshold may cross otherwise-impassable **Valaar** cells, paying an extra per-tick energy drain, and the ability **decays through disuse** (Lamarckian) — two consecutive generations that never swim lose it — with the decay gated behind a config flag.

**Architecture:** A 9th genome scalar `swim` makes an organism *genetically* able. Actual ability (`TraitOrganism::can_swim`) is `swim > 0.5` further gated by a per-organism `swim_disuse` counter against `EcoParams::swim_disuse_limit` (`None` disables the Lamarckian channel). Movement stays terrain-agnostic on **bool masks**: the existing `passability` mask is unchanged (Valaar stays `false`); a new parallel **`swimmable`** mask marks Valaar, and a swimmer may enter a cell if it is passable **or** swimmable. A dedicated `valaar_cost` tick step drains energy from anyone standing on a swimmable cell and marks them as having swum (`swam = true`); `reproduce` propagates the disuse counter (`0` if the parent swam, else `parent + 1`). Once disuse suppresses the ability the lineage physically cannot swim to reset it, so loss is permanent without a fresh mutation.

**Tech Stack:** Rust (edition 2021). No new dependencies. Engine modules only (`genome`, `params`, `organism`, `ecology`, `world`, `mapsim`, `sim`); no viewer changes (swimmers simply render where they now legally stand).

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** `prompt.md` item #1, refined across the 2026-06-20 design discussion. Builds directly on the shipped `docs/plans/2026-06-20-terrain-aware-movement-and-viewer-stats.md` (the `passability` mask and the `move_organisms(..., passable: Option<&[bool]>)` gate this plan extends).

**Builds on (shipped — exact interfaces consumed):**

- `genome::Genome` — `pub const TRAIT_COUNT`; `from_array([f32; TRAIT_COUNT])`, `to_array`, `clamped`, `random`, `mutate`; fields `size, valaar_efficiency, speed, diet, repro_threshold, lifespan, heat_tolerance, drought_tolerance`.
- `organism::{Organism, TraitOrganism}` — `TraitOrganism{ genome, pos, energy, age }` (all pub), `new(genome, pos, energy)`.
- `params::EcoParams` — built only via `EcoParams::default()`; all rates are pub `f32`.
- `ecology::{move_organisms, reproduce, ...}` — `move_organisms<S>(space, field, pop, eco, rng, passable: Option<&[bool]>)`; `reproduce(pop, eco, rng)` builds children via `TraitOrganism::new`.
- `world::World<S>` — pub `space`; `set_passability(Vec<bool>)`, `passability() -> Option<&[bool]>`; `space.len()` is all layers in `Space::index` order.
- `mapsim::world_from_materials(sw, sh, mats: &[CellType]) -> World<Grid2p5D>` — sets sources + the passability mask (surface from `mats`, underground all-passable).
- `sim::Sim<S>::step` — fixed tick order: `absorb → move_organisms → predate → environmental_stress → metabolize → cull_and_recycle → reproduce`.
- `terrain::CellType` — `{ Ocean, Land, Valaar, Rock, Mountain }`, `passable()`.

## Global Constraints

- **Language:** Rust, edition 2021. No new dependencies.
- **Formatting / lint gate (repo convention):** the repo is **not** rustfmt-clean — **do NOT run `cargo fmt`**. Hand-format to match surrounding code (4-space indent, `//!`/`///` docs). Gate every task on `cargo test` and `cargo clippy --all-targets`.
- **Determinism:** no wall-clock or RNG added to rendering/stats. `move_organisms` still draws the RNG exactly once per organism (before any terrain check); `reproduce` still mutates the genome once per child in the same order; `valaar_cost` uses no RNG.
- **Genome literal discipline:** widening `TRAIT_COUNT` to 9 makes every typed `[f32; TRAIT_COUNT]` signature correct for free, but every fixed-length `[f32; 8]` *literal* becomes a compile error the compiler points at. Seeded organisms (the two real seeding sites) get `swim = 0.0` so swimming **arises by mutation**; behaviour-preserving test literals get a neutral `swim = 0.5` (≤ 0.5 ⇒ non-swimmer, so no existing test changes meaning).
- **Version control:** work on `main` (per user request). **Commit per task**, plain message, **no `Co-Authored-By` trailer**. The working tree already carries the user's `params.rs`/`climate.rs`/`terrain.rs` edits — `git add` only the files each task names so those stay out of these commits unless a task legitimately edits `params.rs`.

---

## Design assumptions (flagged for veto)

- **A1 — `swim` is the 9th gene, threshold `0.5`.** Genetically able iff `swim > 0.5` (matches the existing `diet > 0.5` convention). Mutation moves it like any trait.
- **A2 — Lamarckian disuse behind `EcoParams::swim_disuse_limit: Option<u8>`.** `Some(2)` (default) = lose the ability once the lineage reaches 2 consecutive non-swimming generations; `None` = pure genetic, no decay. The `swam` bool and `swim_disuse` counter are *always* tracked; only the suppression effect is gated.
- **A3 — Movement stays on bool masks (no `terrain` dependency in `ecology`).** Keep the shipped `passability` mask (Valaar `false`); add a parallel `swimmable` mask (Valaar `true`). Enterable iff `passable[i] || (can_swim && swimmable[i])`. Ocean/Mountain/Rock are `false` in both ⇒ barred to all.
- **A4 — Swimming is "being on Valaar", charged once per tick.** `valaar_cost` drains `valaar_drain` from any organism whose cell is swimmable and sets `swam = true` there. Movement only *permits* the step; `valaar_cost` is the single place that records use and charges cost (covers both entering and lingering). Default `valaar_drain = 0.1` (10× basal — a placeholder dial).
- **A5 — Loss is permanent per lineage.** Once disuse suppresses the ability the organism cannot enter Valaar, so it cannot reset its counter; re-acquisition requires a fresh `swim` mutation crossing `0.5`.

---

## File Structure

- `src/genome.rs` — **modified**: `TRAIT_COUNT 8→9`, `swim` field, `from_array`/`to_array`, tests.
- `src/params.rs` — **modified**: `valaar_drain`, `swim_disuse_limit` + a defaults test.
- `src/organism.rs` — **modified**: `swam`/`swim_disuse` fields, `SWIM_THRESHOLD`, `can_swim` + tests.
- `src/world.rs` — **modified**: `swimmable` mask field + accessors + test.
- `src/mapsim.rs` — **modified**: `world_from_materials` builds the swimmable mask + test; one seed literal gains `swim`.
- `src/ecology.rs` — **modified**: `move_organisms` gains `swimmable`; new `valaar_cost`; `reproduce` propagates disuse; tests.
- `src/sim.rs` — **modified**: pass `swimmable()` to `move_organisms`; add the `valaar_cost` step.
- `src/population.rs`, `src/bin/life.rs` — **modified**: fix genome literals.
- `README.md` — **modified**: trait count 8→9, note the swim mechanic.

---

### Task 1: Add the `swim` gene (TRAIT_COUNT 8 → 9)

**Files:**
- Modify: `src/genome.rs` (const, field, `from_array`/`to_array`, tests)
- Modify (literal fixes): `src/mapsim.rs`, `src/organism.rs`, `src/population.rs`, `src/ecology.rs`, `src/bin/life.rs`

**Interfaces:**
- Produces: `Genome.swim: f32` at array index `8`; `TRAIT_COUNT == 9`.

- [ ] **Step 1: Write the failing test**

In `src/genome.rs`, change `round_trips_through_array` to 9 elements and add a swim test:

```rust
    #[test]
    fn round_trips_through_array() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);
        assert_eq!(g.to_array(), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);
        assert_eq!(g.diet, 0.4);
        assert_eq!(g.heat_tolerance, 0.7);
        assert_eq!(g.drought_tolerance, 0.8);
    }

    #[test]
    fn swim_round_trips_as_the_ninth_trait() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.95]);
        assert_eq!(g.swim, 0.95);
        assert_eq!(g.to_array()[8], 0.95);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib genome`
Expected: FAIL — `expected an array with a size of 8, found one with a size of 9` and `no field swim on type Genome`.

- [ ] **Step 3: Widen the genome**

In `src/genome.rs`:

```rust
/// Number of scalar traits in the genome.
pub const TRAIT_COUNT: usize = 9;
```

Add the field after `drought_tolerance` in `struct Genome`:

```rust
    /// Resistance to drought (high = needs little water).
    pub drought_tolerance: f32,
    /// Capacity to cross Valaar. Genetically "able to swim" when `> 0.5`; actual
    /// ability is further gated by the Lamarckian disuse counter on the organism
    /// (see `TraitOrganism::can_swim`).
    pub swim: f32,
}
```

In `from_array`, after `drought_tolerance: a[7],`:

```rust
            drought_tolerance: a[7],
            swim: a[8],
        }
```

In `to_array`, after `self.drought_tolerance,`:

```rust
            self.drought_tolerance,
            self.swim,
        ]
```

Update the remaining `genome.rs` test literals (append a 9th value):

```rust
        let g = Genome::from_array([-1.0, 2.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 2.0]).clamped();
        assert_eq!(g.size, 0.0);
        assert_eq!(g.valaar_efficiency, 1.0);
        assert_eq!(g.speed, 0.5);
        assert_eq!(g.swim, 1.0);
```

```rust
        let g = Genome::from_array([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
```

```rust
        let g = Genome::from_array([0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.2, 0.9, 0.5]);
```

- [ ] **Step 4: Fix every other genome literal (compiler-guided)**

Append a `swim` value to each `[f32; 8]` literal. Order is `[size, valaar_efficiency, speed, diet, repro_threshold, lifespan, heat_tolerance, drought_tolerance, swim]`.

`src/mapsim.rs` — the seeded organism gets `swim = 0.0` (swimming arises by mutation); add a line before the closing `])`:

```rust
            0.5,                           // heat_tolerance
            0.5,                           // drought_tolerance
            0.0,                           // swim (arises by mutation)
        ])
        .clamped();
```

`src/mapsim.rs` test literals — append `, 0.5`:

```rust
            Genome::from_array([0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.5, 0.5]), c, 1.0);
```
```rust
            Genome::from_array([0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5]), c, 1.0);
```
```rust
            Genome::from_array([size, 0.5, 0.5, diet, 0.5, 0.5, 0.5, 0.5, 0.5])
```

`src/organism.rs` (`genome_with` helper) — append `, 0.5`:

```rust
        Genome::from_array([size, 0.5, 0.5, 0.5, 0.5, lifespan, 0.5, 0.5, 0.5])
```

`src/population.rs` — switch the count-sized literal to `TRAIT_COUNT` and import it. Change `use crate::genome::Genome;` (in the `tests` module) to:

```rust
    use crate::genome::{Genome, TRAIT_COUNT};
```
and the helper:

```rust
    fn org_at(c: Coord) -> TraitOrganism {
        TraitOrganism::new(Genome::from_array([0.5; TRAIT_COUNT]), c, 1.0)
    }
```

`src/ecology.rs` test literals — append `, 0.5` to each of these:

```rust
        Genome::from_array([0.5, eff, 0.0, diet, 0.9, 0.5, 0.5, 0.5, 0.5])
```
```rust
        Genome::from_array([size, 1.0, 0.0, 1.0, 0.9, 0.5, 0.5, 0.5, 0.5])
```
```rust
        Genome::from_array([size, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5])
```
```rust
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5]);
```
```rust
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
```
```rust
        Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, heat_tol, drought_tol, 0.5])
```

The four identical `[0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5]` literals (in `moves_uphill_toward_richer_valaar`, `at_local_max_it_stays_put`, `does_not_step_onto_impassable_richer_neighbor`, `boxed_in_organism_stays_and_pays_nothing`) each become:

```rust
Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5])
```

`src/bin/life.rs` — the headless seed gets `swim = 0.0`:

```rust
        let g = Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6, 0.6, 0.6, 0.0]);
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add src/genome.rs src/mapsim.rs src/organism.rs src/population.rs src/ecology.rs src/bin/life.rs
git commit -m "feat: add a swim gene as the 9th genome trait"
```

---

### Task 2: `valaar_drain` + `swim_disuse_limit` on `EcoParams`

**Files:**
- Modify: `src/params.rs` (two fields + defaults + a test)

**Interfaces:**
- Produces: `EcoParams.valaar_drain: f32` (default `0.1`); `EcoParams.swim_disuse_limit: Option<u8>` (default `Some(2)`).

> The working tree already has user edits in `params.rs`; this task adds two fields and a tests module without touching the user's retuned values. `git add src/params.rs` will include the user's edits in this commit — that is acceptable (their energy retune is intentional); do **not** revert them.

- [ ] **Step 1: Write the failing test**

Add to the bottom of `src/params.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swim_defaults_are_present() {
        let eco = EcoParams::default();
        assert_eq!(eco.swim_disuse_limit, Some(2));
        assert!((eco.valaar_drain - 0.1).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib params`
Expected: FAIL — `no field swim_disuse_limit on type EcoParams` / `valaar_drain`.

- [ ] **Step 3: Add the fields**

In `src/params.rs`, add to `struct EcoParams` (after `initial_energy`):

```rust
    /// Energy a seeded/initial organism starts with.
    pub initial_energy: f32,
    /// Extra energy drained per tick while standing on a Valaar cell.
    pub valaar_drain: f32,
    /// Consecutive non-swimming generations after which a lineage loses the
    /// swimming ability (Lamarckian use-it-or-lose-it). `None` disables decay.
    pub swim_disuse_limit: Option<u8>,
}
```

In the `Default` impl, after `initial_energy: 4.0,`:

```rust
            initial_energy: 4.0,
            valaar_drain: 0.1,
            swim_disuse_limit: Some(2),
        }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib params && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/params.rs
git commit -m "feat: add valaar_drain and swim_disuse_limit to EcoParams"
```

---

### Task 3: `swam`/`swim_disuse` state + `can_swim` on `TraitOrganism`

**Files:**
- Modify: `src/organism.rs` (fields, `new`, `SWIM_THRESHOLD`, `can_swim`, tests)

**Interfaces:**
- Consumes: `Genome.swim` (Task 1), `EcoParams.swim_disuse_limit` (Task 2).
- Produces: `TraitOrganism.swam: bool`, `TraitOrganism.swim_disuse: u8` (both pub); `TraitOrganism::can_swim(&self, eco: &EcoParams) -> bool`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/organism.rs` (a `swim_genome` helper plus three tests):

```rust
    fn swim_genome(swim: f32) -> Genome {
        Genome::from_array([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, swim])
    }

    #[test]
    fn new_organism_has_not_swum() {
        let o = TraitOrganism::new(swim_genome(0.9), Coord::new(0, 0, Layer::Surface), 1.0);
        assert!(!o.swam);
        assert_eq!(o.swim_disuse, 0);
    }

    #[test]
    fn can_swim_requires_gene_above_threshold() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        assert!(TraitOrganism::new(swim_genome(0.9), pos, 1.0).can_swim(&eco));
        assert!(!TraitOrganism::new(swim_genome(0.4), pos, 1.0).can_swim(&eco));
    }

    #[test]
    fn disuse_suppresses_swimming_at_the_limit() {
        let eco = EcoParams::default(); // swim_disuse_limit = Some(2)
        let pos = Coord::new(0, 0, Layer::Surface);
        let mut o = TraitOrganism::new(swim_genome(0.9), pos, 1.0);
        o.swim_disuse = 1;
        assert!(o.can_swim(&eco), "one idle generation still swims");
        o.swim_disuse = 2;
        assert!(!o.can_swim(&eco), "two idle generations lose it");
    }

    #[test]
    fn disuse_is_ignored_when_limit_is_none() {
        let mut eco = EcoParams::default();
        eco.swim_disuse_limit = None;
        let mut o = TraitOrganism::new(swim_genome(0.9), Coord::new(0, 0, Layer::Surface), 1.0);
        o.swim_disuse = 200;
        assert!(o.can_swim(&eco), "decay off → gene alone decides");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib organism`
Expected: FAIL — `no field swam` / `no method named can_swim`.

- [ ] **Step 3: Add the state and method**

In `src/organism.rs`, add a private threshold const near the top of the file (after the `use` lines):

```rust
/// Genetic threshold above which an organism is *able* to swim.
const SWIM_THRESHOLD: f32 = 0.5;
```

Add fields to `struct TraitOrganism` (after `age`):

```rust
    pub age: u32,
    /// Set true the first time this individual occupies a Valaar cell (used by
    /// the Lamarckian disuse channel; persists for the rest of its life).
    pub swam: bool,
    /// Generations its lineage has gone without swimming (inherited at birth).
    pub swim_disuse: u8,
}
```

Update `new`:

```rust
    pub fn new(genome: Genome, pos: Coord, energy: f32) -> Self {
        TraitOrganism { genome, pos, energy, age: 0, swam: false, swim_disuse: 0 }
    }
```

Add `can_swim` to the `impl TraitOrganism` block (after `new`):

```rust
    /// Whether this organism may currently enter Valaar. Genetically able when
    /// `genome.swim > SWIM_THRESHOLD`; if `eco.swim_disuse_limit` is `Some(lim)`,
    /// the ability is further suppressed once the lineage has gone `lim`
    /// generations without swimming.
    pub fn can_swim(&self, eco: &EcoParams) -> bool {
        if self.genome.swim <= SWIM_THRESHOLD {
            return false;
        }
        match eco.swim_disuse_limit {
            Some(lim) => self.swim_disuse < lim,
            None => true,
        }
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib organism && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/organism.rs
git commit -m "feat: track swim state and add TraitOrganism::can_swim"
```

---

### Task 4: `swimmable` mask on `World`

**Files:**
- Modify: `src/world.rs` (field + accessors + test)

**Interfaces:**
- Produces: `World::set_swimmable(&mut self, mask: Vec<bool>)` (debug-asserts `len == space.len()`); `World::swimmable(&self) -> Option<&[bool]>` (`None` until set).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/world.rs`:

```rust
    #[test]
    fn swimmable_defaults_none_and_round_trips() {
        let space = Grid2p5D::new(2, 2);
        let mut world = World::new(space, Params::default());
        assert!(world.swimmable().is_none());
        let mut mask = vec![false; world.space.len()];
        mask[0] = true;
        world.set_swimmable(mask);
        assert_eq!(world.swimmable().unwrap().len(), world.space.len());
        assert!(world.swimmable().unwrap()[0]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib world`
Expected: FAIL — `no method named swimmable`.

- [ ] **Step 3: Add the field and accessors**

In `src/world.rs`, add to `struct World<S>` (after `passability`):

```rust
    passability: Option<Vec<bool>>,
    /// Per-cell "swimmable" mask (sized to `space.len()`): `true` where a
    /// swimming organism may enter despite the cell being impassable in
    /// `passability` (i.e. Valaar). `None` = nothing is swimmable.
    swimmable: Option<Vec<bool>>,
}
```

In `World::new`, add `swimmable: None,` after `passability: None,`.

Add accessors after the `passability` ones:

```rust
    /// Install a per-cell swimmable mask (`true` = a swimmer may enter). Length
    /// must equal `space.len()` (all layers, `Space::index` order).
    pub fn set_swimmable(&mut self, mask: Vec<bool>) {
        debug_assert_eq!(mask.len(), self.space.len(), "mask must cover every cell");
        self.swimmable = Some(mask);
    }

    /// The swimmable mask, if one was installed.
    pub fn swimmable(&self) -> Option<&[bool]> {
        self.swimmable.as_deref()
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib world && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/world.rs
git commit -m "feat: store an optional swimmable mask on World"
```

---

### Task 5: Build the swimmable mask from the material map

**Files:**
- Modify: `src/mapsim.rs` (`world_from_materials` + a test)

**Interfaces:**
- Consumes: `World::set_swimmable` (Task 4), `CellType::Valaar`.
- Produces: `world_from_materials` returns a `World` whose `swimmable()` is `Some` (surface Valaar `true`, everything else `false`, underground all `false`).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/mapsim.rs` (the `grid` helper maps `'O'→Ocean`, `'V'→Valaar`, else `Land`):

```rust
    #[test]
    fn world_marks_only_valaar_swimmable() {
        let (w, h, m) = grid(&["OVL"]); // ocean, valaar, land
        let world = world_from_materials(w, h, &m);
        let mask = world.swimmable().expect("swimmable mask installed");
        let idx = |x: u32| world.space.index(Coord::new(x, 0, Layer::Surface));
        assert!(!mask[idx(0)], "ocean is not swimmable");
        assert!(mask[idx(1)], "valaar is swimmable");
        assert!(!mask[idx(2)], "land is not swimmable");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib mapsim`
Expected: FAIL — `swimmable mask installed` panics (None).

- [ ] **Step 3: Build and install the mask**

In `src/mapsim.rs` `world_from_materials`, after the existing `world.set_passability(mask);` line and before `world`:

```rust
    world.set_passability(mask);
    let mut swim = vec![false; world.space.len()]; // underground: nothing swimmable
    for i in 0..plane {
        swim[i] = mats[i] == CellType::Valaar; // only Valaar opens to swimmers
    }
    world.set_swimmable(swim);
    world
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib mapsim && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/mapsim.rs
git commit -m "feat: mark Valaar cells swimmable in world_from_materials"
```

---

### Task 6: Terrain-aware `move_organisms`

**Files:**
- Modify: `src/ecology.rs` (signature + body; update four test calls; add two tests + a `swimmer` helper)
- Modify: `src/sim.rs` (pass the swimmable mask)

**Interfaces:**
- Consumes: `TraitOrganism::can_swim` (Task 3), `World::swimmable` (Task 4).
- Produces: `move_organisms<S>(space, field, pop, eco, rng, passable: Option<&[bool]>, swimmable: Option<&[bool]>)`. A cell is enterable iff `passable[i]` (or `passable` is `None`) **or** the organism `can_swim` and `swimmable[i]`.

- [ ] **Step 1: Update the four existing move-test calls**

In `src/ecology.rs`, add `, None` (the new `swimmable` arg) to every existing `move_organisms(...)` call in the tests `moves_uphill_toward_richer_valaar`, `at_local_max_it_stays_put`, `does_not_step_onto_impassable_richer_neighbor`, `boxed_in_organism_stays_and_pays_nothing`. For example:

```rust
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, None, None);
```
and (the masked ones):
```rust
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&mask), None);
```

- [ ] **Step 2: Add the `swimmer` helper and two new tests**

Add to the `tests` module in `src/ecology.rs`:

```rust
    // [size, eff, speed, diet, repro, lifespan, heat_tol, drought_tol, swim]
    fn swimmer(swim: f32) -> Genome {
        Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, swim])
    }

    #[test]
    fn swimmer_enters_richer_valaar_cell() {
        let space = Grid2p5D::new(2, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 1.0);
        field.set(space.index(Coord::new(1, 0, Layer::Surface)), 9.0); // richer
        let mut passable = vec![true; space.len()];
        let mut swimmable = vec![false; space.len()];
        let valaar = space.index(Coord::new(1, 0, Layer::Surface));
        passable[valaar] = false; // impassable to walkers
        swimmable[valaar] = true; // but it is Valaar
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.9), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), Some(&swimmable));
        assert_eq!(pop.organisms()[0].pos, Coord::new(1, 0, Layer::Surface), "a swimmer crosses into valaar");
    }

    #[test]
    fn non_swimmer_is_blocked_from_valaar() {
        let space = Grid2p5D::new(2, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 1.0);
        field.set(space.index(Coord::new(1, 0, Layer::Surface)), 9.0);
        let mut passable = vec![true; space.len()];
        let mut swimmable = vec![false; space.len()];
        let valaar = space.index(Coord::new(1, 0, Layer::Surface));
        passable[valaar] = false;
        swimmable[valaar] = true;
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.1), start, 5.0)); // gene below threshold
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), Some(&swimmable));
        assert_eq!(pop.organisms()[0].pos, start, "a non-swimmer cannot enter valaar");
    }
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: FAIL — `move_organisms` arity mismatch (compile error).

- [ ] **Step 4: Update `move_organisms`**

In `src/ecology.rs`, replace the whole function:

```rust
/// Each organism moves with probability `speed` toward its richest in-bounds,
/// **enterable** planar neighbour (gradient ascent on valaar). Moving costs
/// `move_cost·speed`. A non-swimmer may enter a cell only where `passable`
/// allows (`None` = no terrain constraint); a swimmer (`can_swim`) may also
/// enter a `swimmable` cell (Valaar). Cells barred to all (ocean/mountain/rock)
/// are false in both masks. Neighbours never cross layers.
pub fn move_organisms<S: Space>(
    space: &S,
    field: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
    passable: Option<&[bool]>,
    swimmable: Option<&[bool]>,
) {
    for o in pop.organisms_mut() {
        // Draw first so the rng stream advances once per organism regardless.
        if rng.next_unit() >= o.genome.speed {
            continue;
        }
        let can_swim = o.can_swim(eco);
        let mut best = o.pos;
        let mut best_v = field.get(space.index(o.pos));
        for n in space.planar_neighbors(o.pos) {
            let ni = space.index(n);
            let open = match passable {
                Some(m) => m[ni],
                None => true,
            };
            let swim_ok = can_swim
                && match swimmable {
                    Some(m) => m[ni],
                    None => false,
                };
            if !open && !swim_ok {
                continue; // impassable terrain blocks the step
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

In `src/sim.rs`, update the `move_organisms` call to pass the swimmable mask:

```rust
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
            self.world.passability(),
            self.world.swimmable(),
        );
```

- [ ] **Step 6: Run the full suite**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 7: Commit**

```bash
git add src/ecology.rs src/sim.rs
git commit -m "feat: let swimmers cross Valaar in move_organisms"
```

---

### Task 7: `valaar_cost` tick step

**Files:**
- Modify: `src/ecology.rs` (new function + two tests; reuses the `swimmer` helper from Task 6)

**Interfaces:**
- Consumes: `EcoParams.valaar_drain` (Task 2), `World::swimmable` (Task 4), `TraitOrganism.swam`.
- Produces: `valaar_cost<S: Space>(space, swimmable: Option<&[bool]>, pop, eco)` — for each organism on a swimmable cell, sets `swam = true` and subtracts `eco.valaar_drain`. `None` = no-op.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn valaar_cell_drains_and_marks_swam() {
        let space = Grid2p5D::new(2, 1);
        let eco = EcoParams::default();
        let mut swimmable = vec![false; space.len()];
        swimmable[space.index(Coord::new(1, 0, Layer::Surface))] = true;
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.9), Coord::new(1, 0, Layer::Surface), 5.0));
        valaar_cost(&space, Some(&swimmable), &mut pop, &eco);
        assert!((pop.organisms()[0].energy - (5.0 - eco.valaar_drain)).abs() < 1e-6);
        assert!(pop.organisms()[0].swam, "standing on valaar counts as swimming");
    }

    #[test]
    fn dry_land_is_free_and_not_swimming() {
        let space = Grid2p5D::new(2, 1);
        let eco = EcoParams::default();
        let mut swimmable = vec![false; space.len()];
        swimmable[space.index(Coord::new(1, 0, Layer::Surface))] = true;
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.9), Coord::new(0, 0, Layer::Surface), 5.0)); // on land
        valaar_cost(&space, Some(&swimmable), &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 5.0);
        assert!(!pop.organisms()[0].swam);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: FAIL — `cannot find function valaar_cost`.

- [ ] **Step 3: Add the function**

In `src/ecology.rs`, add (next to the other tick functions):

```rust
/// Drain `eco.valaar_drain` from every organism standing on a swimmable
/// (Valaar) cell, and mark it as having swum this life (`swam = true`). Only
/// swimmers can legally be on such cells, so this is the running cost of
/// swimming. `swimmable`: `None` = no Valaar anywhere (no-op).
pub fn valaar_cost<S: Space>(
    space: &S,
    swimmable: Option<&[bool]>,
    pop: &mut Population,
    eco: &EcoParams,
) {
    let mask = match swimmable {
        Some(m) => m,
        None => return,
    };
    for o in pop.organisms_mut() {
        if mask[space.index(o.pos)] {
            o.swam = true;
            o.energy -= eco.valaar_drain;
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib ecology && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/ecology.rs
git commit -m "feat: add valaar_cost — energy drain + swam mark on Valaar"
```

---

### Task 8: Lamarckian disuse inheritance in `reproduce`

**Files:**
- Modify: `src/ecology.rs` (`reproduce` body + two tests)

**Interfaces:**
- Consumes: `TraitOrganism.{swam, swim_disuse}` (Task 3), `TraitOrganism::can_swim` (Task 3).
- Produces: children whose `swim_disuse` is `0` if the parent swam, else `parent.swim_disuse + 1` (saturating); `swam` starts `false`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn swimmer_parent_resets_child_disuse() {
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut pop = Population::new();
        // repro_threshold 0 ⇒ any energy reproduces; swim gene high.
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.9]);
        let mut parent = TraitOrganism::new(g, c, 5.0);
        parent.swam = true;
        parent.swim_disuse = 1;
        pop.spawn(parent);
        let mut rng = Rng::new(3);
        reproduce(&mut pop, &eco, &mut rng);
        assert_eq!(pop.organisms()[1].swim_disuse, 0, "a swimming parent resets the counter");
        assert!(!pop.organisms()[1].swam, "the child has not swum yet");
    }

    #[test]
    fn idle_parent_increments_child_disuse_to_loss() {
        let eco = EcoParams::default(); // limit Some(2)
        let c = Coord::new(1, 1, Layer::Surface);
        let mut pop = Population::new();
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.9]);
        let mut parent = TraitOrganism::new(g, c, 5.0);
        parent.swam = false;
        parent.swim_disuse = 1;
        pop.spawn(parent);
        let mut rng = Rng::new(3);
        reproduce(&mut pop, &eco, &mut rng);
        let child = &pop.organisms()[1];
        assert_eq!(child.swim_disuse, 2);
        assert!(!child.can_swim(&eco), "two idle generations lose the ability");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: FAIL — child `swim_disuse` is `0` (inherited default), not the propagated value.

- [ ] **Step 3: Propagate the counter**

In `src/ecology.rs` `reproduce`, replace the child-construction block:

```rust
        if o.energy >= threshold && o.energy > 0.0 {
            let child_energy = o.energy * eco.repro_cost_fraction;
            o.energy -= child_energy;
            let child_genome = o.genome.mutate(rng, eco.mutation_rate);
            let mut child = TraitOrganism::new(child_genome, o.pos, child_energy);
            // Lamarckian: reset disuse if the parent actually swam this life,
            // otherwise carry it forward incremented.
            child.swim_disuse = if o.swam { 0 } else { o.swim_disuse.saturating_add(1) };
            children.push(child);
        }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib ecology && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/ecology.rs
git commit -m "feat: propagate swim-disuse to offspring in reproduce"
```

---

### Task 9: Wire `valaar_cost` into the tick + integration check

**Files:**
- Modify: `src/sim.rs` (add the `valaar_cost` step)

**Interfaces:**
- Consumes: `ecology::valaar_cost` (Task 7), `World::swimmable` (Task 4).
- Produces: the live tick now charges swimmers and records use, so the disuse channel actually advances.

> No new unit test — this composes already-tested pieces; verified by the full suite + clippy + a manual run.

- [ ] **Step 1: Add the step**

In `src/sim.rs` `step`, insert `valaar_cost` after `environmental_stress` and before `metabolize`:

```rust
        ecology::environmental_stress(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::valaar_cost(&self.world.space, self.world.swimmable(), &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
```

- [ ] **Step 2: Run the full suite + clippy**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 3: Manual smoke check**

Run: `cargo run --release --bin mapview`
Expected: the sim runs; over time some organisms (those that mutate `swim > 0.5`) appear **on the Valaar rivers/reservoir** rather than strictly avoiding them — confirming swimmers cross. (No swimmers at tick 0: seeds start `swim = 0.0`.)

- [ ] **Step 4: Commit**

```bash
git add src/sim.rs
git commit -m "feat: charge swimmers each tick via valaar_cost in Sim::step"
```

---

### Task 10: Document the swim trait

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the trait count and add a mechanic note**

In `README.md`, change the file-tree line for `genome.rs` from `trait-vector genome (8 traits)` to:

```
  genome.rs       trait-vector genome (9 traits, incl. swim)
```

And in the gene↔world coupling note (the `valaar↔valaar_efficiency, heat↔heat_tolerance, water↔drought_tolerance` line), append swimming:

```markdown
  water↔`drought_tolerance`. A `swim` gene lets life cross Valaar (impassable to
  others) at an extra energy cost; unused, it decays over ~2 generations
  (Lamarckian, toggled by `EcoParams::swim_disuse_limit`).
```

- [ ] **Step 2: Lint check and commit**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

```bash
git add README.md
git commit -m "docs: note the swim trait and Valaar crossing"
```

---

## Self-Review

**Spec coverage (prompt.md #1):**
- "swimming trait … able to swim across the valaar" → Tasks 1 (gene), 3 (`can_swim`), 5 (swimmable mask), 6 (movement gate). ✓
- "staying in valaar drains more of their energy (tunable parameter)" → Task 2 (`valaar_drain`), Task 7 (`valaar_cost`), Task 9 (wired into tick). ✓
- "if 2 generations have not swum the offspring will not inherit the trait" → Tasks 3 (`swim_disuse` gate), 7 (`swam` mark), 8 (propagation). ✓
- "put the lamarckian disuse behind a flag" → `EcoParams::swim_disuse_limit: Option<u8>` (`None` disables), Tasks 2/3. ✓
- TRAIT_COUNT as single source of truth for array length → Task 1 (`population.rs` uses `[0.5; TRAIT_COUNT]`; all typed signatures auto-size). ✓

**Placeholder scan:** every code step shows full code; every literal fix is enumerated with its file; run steps give command + expected result. No TBD/TODO. ✓

**Type consistency:** `swim: f32` at index 8 (Tasks 1/3/6/8 literals all 9-wide); `can_swim(&self, eco: &EcoParams) -> bool` (Task 3) consumed in Tasks 6/8; `swimmable: Option<&[bool]>` consistent across `World` (Task 4), `mapsim` (Task 5), `move_organisms`/`valaar_cost` (Tasks 6/7), `Sim::step` (Tasks 6/9); `swim_disuse: u8` set in Task 3, propagated saturating in Task 8; `valaar_drain: f32`/`swim_disuse_limit: Option<u8>` (Task 2) used in Tasks 3/7. ✓

**Determinism:** `move_organisms` still draws RNG once per organism before the terrain check (Task 6); `valaar_cost` is RNG-free (Task 7); `reproduce` mutation order unchanged (Task 8). ✓

**Deferred (not in this plan):** seeding-policy tuning (swimmers are seeded at `swim = 0.0`, so the trait is mutation-only at start); conductivity-weighted diffusion; any viewer change to *highlight* swimmers (they already render via existing organism drawing).
