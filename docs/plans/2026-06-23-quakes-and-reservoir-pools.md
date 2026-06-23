# Quakes & Reservoir Pools Implementation Plan (P1 of the Dusk-survival redesign)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repurpose the underground valaar reservoirs from a *dig-down* refuge into a *quake-up* lifeline — remove the `dig` gene and its descent mechanic, turn each reservoir into one solid underground pool, and add **earthquakes** that, during **Vraze**, burst a pool's accumulated valaar up to the surface so the valaar-poor Dusk gets an annual valaar pulse.

**Architecture:** A new `quakes` module mirrors the existing `bridges` controller exactly — a dedicated RNG (so the ecology stream is untouched and old seeds stay reproducible), a per-Vraze schedule, per-pool active windows. The burst is a one-shot underground→surface valaar transfer at the moment a pool becomes active; the active window is how long the surface fracture stays visible. Reservoirs are placed as solid contiguous pools (`mapsim::place_reservoir_pools`) and registered as slow underground **sources** (each pool refills between quakes) — with **no** access points and **no** descendable mask, so the only way up is a quake. The viewer draws the fracture/burst over active pools.

**Tech Stack:** Rust (edition 2021); `std`-only engine core; `eframe`/`egui` for the viewer. **No new crates.**

**Depends on:** the shipped reservoir placement (plan 6) and the `Bridges` controller (`src/bridges.rs`), which this plan reuses as the template. This is **P1** of a two-plan redesign; **P2** (multi-energy: heat/water offset-feeding + a `valaar_reliance` gene) is a separate later plan and is **out of scope here**.

## Global Constraints

- **Language:** Rust, edition 2021. **No new dependencies.**
- **Engine stays `std`-only:** `genome`, `organism`, `ecology`, `world`, `mapsim`, `quakes` must not pull in `eframe`/`egui`/`serde`. Viewer/egui code lives only in `src/bin/mapview.rs`.
- **Implicit selection only:** nothing scores fitness. The quake pulse spreads (or not) because life that happens to sit near an erupting pool survives the lean seasons and reproduces — never because a fitness term rewards it.
- **Determinism:** identical seed + params ⇒ identical run. Quakes use a **dedicated RNG** (`QUAKE_SEED`), exactly like `Bridges`, so the main ecology `Rng` stream advances identically. Removing the `dig` gene shrinks the genome array (10 → 9), which changes how many `Rng` draws `Genome::random`/`mutate` consume — so runs differ from pre-P1 runs. **Expected.**
- **Valaar is the one currency:** the reservoirs hold valaar; the quake just moves it from the Underground layer up to the Surface. No second currency is introduced in P1.
- **Formatting:** do **not** run `cargo fmt` (repo isn't rustfmt-clean). Hand-format; gate on `cargo clippy --all-targets`.
- **Version control:** commit per task. Plain commit messages, **no `Co-Authored-By` trailer**.
- **Visual-rep convention:** the earthquake's surface fracture + valaar burst (Task 6) is this mechanic's viewer visual — part of "done".

---

## Design assumptions (flagged for veto)

- **D1 — `dig` is removed entirely**, not left inert. `TRAIT_COUNT` goes 10 → 9; the genome's 10th trait (`dig`, index 9) and `TraitOrganism::can_dig`/`DIG_THRESHOLD`/`EcoParams::dig_drain` all go. The descent move candidate in `move_organisms` and the `World` `descendable` mask go with them. The Underground stays a real layer (reservoirs are field cells there) but is **no longer a habitat** — nothing descends, so the `environmental_stress` Underground exemption becomes dead-but-harmless and is **left as-is** (removing it risks an unused-`Layer` churn for zero behaviour change).
- **D2 — reservoirs become solid pools.** Same anchors as plan 6 (south-west quadrant of the major continent + the southmost island), but the southern band is filled **solidly** (every land cell in the band, no stride) into one `ReservoirPool` per region, instead of scattered columns.
- **D3 — pools are underground sources only.** Each pool cell is registered as an Underground valaar **source** (slow injection — the thawing sunken crystal) so the pool refills between quakes. **No** access points and **no** descendable mask are registered — valaar stays trapped below until a quake releases it. (`World::add_access_point`/`exchange_layers` remain as a general engine feature, just unused by reservoirs.)
- **D4 — the earthquake season is Vraze** (arh index 3) — canon (`world-design.md`: *"earthquakes … peaking Vraze … underground pools become exposed"*). Quakes share the season with land bridges; both are "Vraze instability". They are independent controllers.
- **D5 — burst-to-surface.** On the tick a pool becomes active, `release_fraction` of each pool cell's *underground* valaar is transferred to the **surface** cell directly above it (`valaar.add(surface, +amt); valaar.add(underground, -amt)`). One-shot at eruption; the active window only governs how long the fracture is drawn. The burst happens **before** `absorb` in the tick so organisms can eat the pulse the same tick.
- **D6 — `Quakes` mirrors `Bridges`.** Dedicated RNG, `schedule()` rolled at the first Vraze tick of each year, per-pool `(erupt_craw, end_craw)` window, `update()` flips active state. Defaults: `erupt_fraction 0.6`, `release_fraction 0.8`, `min_duration 20`, `max_duration 40` craws.
- **D7 — the reservoir+quake install is behind the existing feature flag.** The plan-6 `ENABLE_UNDERGROUND_RESERVOIRS` flag in `mapview.rs` is renamed `ENABLE_RESERVOIRS_AND_QUAKES` and now gates both the pool sources and the `Quakes` controller. Default **`true`** (this is the feature).

---

## File Structure

- `src/genome.rs` — **modified**: drop `dig` (10th trait), `TRAIT_COUNT` 10 → 9, `from_array`/`to_array`, the dig round-trip test.
- `src/organism.rs` — **modified**: drop `DIG_THRESHOLD`, `can_dig`, the dig test + `dig_genome` helper.
- `src/params.rs` — **modified**: drop `dig_drain` + its test.
- `src/ecology.rs` — **modified**: `move_organisms` drops the `descendable` param + the `can_dig` descent candidate + the `dig_drain` charge; drop the descent test.
- `src/world.rs` — **modified**: drop the `descendable` mask (field + `set_descendable`/`descendable` + its test).
- `src/sim.rs` — **modified**: drop the `descendable()` arg to `move_organisms`; own an `Option<Quakes>` (`set_quakes`), drive it in `step`, expose `quake_active_cells`.
- `src/quakes.rs` — **created**: `ReservoirPool`, `QuakeConfig`, `QuakeUpdate`, `Quakes`.
- `src/lib.rs` — **modified**: `pub mod quakes;`.
- `src/mapsim.rs` — **modified**: `region_cells`, `place_reservoir_pools` → `Vec<ReservoirPool>`; `add_underground_reservoirs` registers Underground sources only; replace the reservoir tests; **remove** `place_underground_reservoirs`/`gradient_columns` column machinery.
- `src/bin/mapview.rs` — **modified**: build pools + `Quakes`, rename the feature flag, remove the `dig_drain` slider, add `QuakeConfig` tunables + sliders, add `draw_quakes`.
- `~/dev/ideas/world-design.md` — **modified**: a one-line note that quakes (not digging) are the reservoir-access mechanic.
- `README.md` — **modified**: status rows.

Many test files carry 9- and 10-element `Genome::from_array([...])` literals (27 sites across `genome`, `organism`, `ecology`, `mapsim`, `population`, `bin/life`); Task 2 shortens every one — the compiler flags each when `TRAIT_COUNT` changes.

---

### Task 1: Remove the descent mechanic (the `dig` gene goes inert)

Remove everything that *reads or wires* `dig`, leaving the `dig` field itself in `Genome` (still index 9) so the genome array width is unchanged — Task 2 removes the field. This keeps each task compiling.

**Files:**
- Modify: `src/organism.rs`, `src/params.rs`, `src/ecology.rs`, `src/world.rs`, `src/sim.rs`, `src/mapsim.rs`
- (No new tests; deletes obsolete ones.)

**Interfaces:**
- Produces: `move_organisms(space, field, pop, eco, rng, passable, swimmable)` — the `descendable` parameter is gone. `add_underground_reservoirs(world, cols)` now registers Underground sources only.

- [ ] **Step 1: `src/organism.rs` — drop the dig threshold, `can_dig`, and its tests**

Delete the `DIG_THRESHOLD` const (the `/// Genetic threshold … to dig` doc + `const DIG_THRESHOLD: f32 = 0.5;`), the whole `can_dig` method (its doc comment + body), the `dig_genome` test helper, and the `can_dig_requires_gene_above_threshold` test. Leave `swim`/`can_swim` untouched.

- [ ] **Step 2: `src/params.rs` — drop `dig_drain`**

Remove the `dig_drain` field (and its doc line), its initialiser in `Default`, and the `dig_drain_default_is_present` test.

- [ ] **Step 3: `src/ecology.rs` — strip the descent candidate from `move_organisms`**

In `move_organisms`: remove the `descendable: Option<&[bool]>` parameter; remove the `let mut best_descend = false;` line; remove the entire `if o.can_dig() { … }` block (lines registering the layer-switch candidate); and in the final move-commit block remove the `if best_descend { o.energy -= eco.dig_drain; }` lines. Update the doc comment to drop the "digger/descendable" sentences. The `#[allow(clippy::too_many_arguments)]` may now be unnecessary — remove it if clippy stops warning (it has 7 args after the change; keep the allow only if clippy complains).

Delete the descent test in the `tests` module: `digger_descends_toward_a_rich_reservoir` (and any `descendable`/`set_descendable` test scaffolding it uses).

- [ ] **Step 4: `src/world.rs` — drop the `descendable` mask**

Remove the `descendable: Option<Vec<bool>>` field (+ its doc), its `None` initialiser in `World::new`, the `set_descendable` and `descendable` methods (+ docs), and the `descendable_defaults_none_and_round_trips` test. Leave `passability`/`swimmable` and the `access_points` API untouched.

- [ ] **Step 5: `src/sim.rs` — drop the `descendable()` argument**

In `Sim::step`, change the `move_organisms(…)` call to drop the final `self.world.descendable(),` argument:

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

- [ ] **Step 6: `src/mapsim.rs` — reservoirs register Underground sources only**

Replace `add_underground_reservoirs` with the source-only version (drop the `desc` vec, the `add_access_point`, and the `set_descendable`):

```rust
/// Register each reservoir column on the Underground layer as a valaar
/// **source** (slow injection — the thawing sunken crystal). No access points
/// or descendable mask: valaar stays below until a quake releases it.
pub fn add_underground_reservoirs(world: &mut World<Grid2p5D>, cols: &[(u32, u32)]) {
    for &(x, y) in cols {
        world.add_source(Coord::new(x, y, Layer::Underground));
    }
}
```

Update the test `add_reservoirs_registers_sources_access_points_and_descendable` → rename to `add_reservoirs_registers_underground_sources` and assert only that the sources are registered:

```rust
    #[test]
    fn add_reservoirs_registers_underground_sources() {
        let mut world = World::new(Grid2p5D::new(4, 4), Params::default());
        add_underground_reservoirs(&mut world, &[(1, 1)]);
        assert_eq!(world.sources(), &[Coord::new(1, 1, Layer::Underground)]);
        assert!(world.access_points().is_empty());
    }
```

- [ ] **Step 7: Run tests + clippy to verify the strip compiles green**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS. The `dig` field still exists on `Genome` (inert); nothing reads it.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: remove the dig descent mechanic (dig gene now inert)"
```

---

### Task 2: Remove the inert `dig` gene (genome 10 → 9)

**Files:**
- Modify: `src/genome.rs` and every file with a `Genome::from_array([…])` literal (compiler-driven): `src/organism.rs`, `src/ecology.rs`, `src/mapsim.rs`, `src/population.rs`, `src/bin/life.rs`.

**Interfaces:**
- Produces: `pub const TRAIT_COUNT: usize = 9;` and a `Genome` with no `dig` field (index 8 = `swim` is the last trait).

- [ ] **Step 1: Edit the dig round-trip test to the new width (it will fail to compile first)**

In `src/genome.rs` `tests`, replace `dig_round_trips_as_the_tenth_trait` with a guard that swim is now the last trait:

```rust
    #[test]
    fn trait_count_is_nine_and_swim_is_last() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);
        assert_eq!(g.swim, 0.9);
        assert_eq!(g.to_array()[8], 0.9);
        assert_eq!(TRAIT_COUNT, 9);
    }
```

- [ ] **Step 2: `src/genome.rs` — drop the field, shrink the count and arrays**

Set `pub const TRAIT_COUNT: usize = 9;`. Remove the `dig` field (+ its doc) from `struct Genome`. Remove `dig: a[9]` from `from_array` (now ends `swim: a[8]`). Remove `self.dig` from the `to_array` array (now ends `self.swim`). Shorten the other genome unit-test literals in this file (e.g. the `to_array` round-trip and `clamped`/`random` tests) from 10 to 9 elements by dropping the trailing value.

- [ ] **Step 3: Run the library genome tests to verify they fail elsewhere by type error**

Run: `cargo build 2>&1 | head -40`
Expected: a list of `mismatched types … expected an array with a fixed size of 9 elements, found one with 10` at every stale `from_array([…])` literal. This list is your worklist.

- [ ] **Step 4: Shorten every stale `from_array([…])` literal from 10 → 9 elements**

For each compiler error, drop the **last** element of the literal (the old `dig` slot). Known sites:
- `src/organism.rs`: `genome_with` (`[size, 0.5, 0.5, 0.5, 0.5, lifespan, 0.5, 0.5, 0.5]`), `swim_genome` (`[0.5, …, swim]` — already ends in `swim`, just drop the trailing `0.5`). (The `dig_genome` helper was deleted in Task 1.)
- `src/ecology.rs`: the `genome` helper (`[0.5, eff, 0.0, diet, 0.9, 0.5, 0.5, 0.5, 0.5]`) and the four `[0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5]` literals.
- `src/mapsim.rs`: the `[…, 0.0, …]`/`[…, 1.0, …]` diet literals and the `genome` helper (`[size, 0.5, 0.5, diet, 0.5, 0.5, 0.5, 0.5, 0.5]`).
- `src/population.rs`, `src/bin/life.rs`: any `from_array([…])` literals — drop the trailing element.

Also fix any non-test 10-element array used to build a genome (none expected outside `from_array`).

- [ ] **Step 5: Run tests + clippy**

Run: `cargo test && cargo clippy --all-targets`
Expected: PASS. `grep -rn "from_array(\[" src | grep -c "0\.5\]"` etc. should show no 10-element literals remain (spot-check: `cargo build` is clean).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: remove the dig gene — genome is now 9 traits"
```

---

### Task 3: The `quakes` module — `ReservoirPool`, `QuakeConfig`, `Quakes`

A self-contained controller, TDD'd in isolation against a tiny `Grid2p5D` + `Field`. No `mapsim`/`sim` dependency yet.

**Files:**
- Create: `src/quakes.rs`
- Modify: `src/lib.rs` (`pub mod quakes;`)

**Interfaces:**
- Produces:
  - `pub struct ReservoirPool { pub cells: Vec<(u32, u32)> }`
  - `pub struct QuakeConfig { pub erupt_fraction: f32, pub release_fraction: f32, pub min_duration: u32, pub max_duration: u32 }` + `Default`
  - `pub struct QuakeUpdate { pub erupted: Vec<(u32, u32)> }`
  - `pub struct Quakes` with `new(pools, cfg, seed) -> Self`, `pool_count() -> usize`, `active_cells() -> Vec<(u32, u32)>`, `update<S: Space>(&mut self, &Calendar, &S, &mut Field) -> QuakeUpdate`.

- [ ] **Step 1: Declare the module**

In `src/lib.rs`, add after `pub mod bridges;`:

```rust
pub mod quakes;
```

- [ ] **Step 2: Write the failing tests**

Create `src/quakes.rs` with only the `#[cfg(test)]` module first (the types come next step). Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Field;
    use crate::season::{Calendar, CRAWS_PER_ARH};
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    fn cal_at(target_craw: u32) -> Calendar {
        let mut c = Calendar::new();
        while c.craw() < target_craw {
            c.advance();
        }
        c
    }

    fn one_pool() -> Vec<ReservoirPool> {
        vec![ReservoirPool { cells: vec![(0, 0)] }]
    }

    #[test]
    fn quiet_outside_vraze() {
        let space = Grid2p5D::new(1, 1);
        let mut valaar = Field::zeros(space.len());
        valaar.set(space.index(Coord::new(0, 0, Layer::Underground)), 1.0);
        let cfg = QuakeConfig { erupt_fraction: 1.0, ..QuakeConfig::default() };
        let mut q = Quakes::new(one_pool(), cfg, 7);
        let upd = q.update(&cal_at(150), &space, &mut valaar); // Goscon
        assert!(upd.erupted.is_empty());
        // nothing moved
        assert_eq!(valaar.get(space.index(Coord::new(0, 0, Layer::Underground))), 1.0);
        assert_eq!(valaar.get(space.index(Coord::new(0, 0, Layer::Surface))), 0.0);
    }

    #[test]
    fn erupts_and_bursts_during_vraze() {
        let space = Grid2p5D::new(1, 1);
        let ui = space.index(Coord::new(0, 0, Layer::Underground));
        let si = space.index(Coord::new(0, 0, Layer::Surface));
        let mut valaar = Field::zeros(space.len());
        valaar.set(ui, 1.0);
        let cfg = QuakeConfig {
            erupt_fraction: 1.0,
            release_fraction: 1.0,
            min_duration: 10,
            max_duration: 10,
        };
        let mut q = Quakes::new(one_pool(), cfg, 7);
        let mut erupted_count = 0;
        let mut c = cal_at(3 * CRAWS_PER_ARH); // start of Vraze
        for _ in 0..CRAWS_PER_ARH {
            let upd = q.update(&c, &space, &mut valaar);
            if !upd.erupted.is_empty() {
                assert_eq!(upd.erupted, vec![(0, 0)]);
                erupted_count += 1;
            }
            c.advance();
        }
        assert_eq!(erupted_count, 1, "one eruption this Vraze");
        assert!(valaar.get(si) > 0.99, "valaar burst up to the surface");
        assert!(valaar.get(ui) < 0.01, "underground pool drained");
    }

    #[test]
    fn schedule_is_seed_deterministic() {
        let space = Grid2p5D::new(1, 1);
        let ui = space.index(Coord::new(0, 0, Layer::Underground));
        let cfg = QuakeConfig { erupt_fraction: 0.5, ..QuakeConfig::default() };
        let run = |seed| {
            let mut valaar = Field::zeros(space.len());
            valaar.set(ui, 1.0);
            let mut q = Quakes::new(
                vec![
                    ReservoirPool { cells: vec![(0, 0)] },
                    ReservoirPool { cells: vec![(0, 0)] },
                    ReservoirPool { cells: vec![(0, 0)] },
                ],
                cfg,
                seed,
            );
            let mut log = Vec::new();
            let mut c = cal_at(3 * CRAWS_PER_ARH);
            for _ in 0..CRAWS_PER_ARH {
                log.push(q.update(&c, &space, &mut valaar).erupted);
                c.advance();
            }
            log
        };
        assert_eq!(run(123), run(123));
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test --lib quakes`
Expected: FAIL to compile (`cannot find type ReservoirPool`).

- [ ] **Step 4: Write the module**

Prepend to `src/quakes.rs` (above the test module):

```rust
//! Earthquakes: a Vraze-season disturbance that fractures the crust over a deep
//! valaar reservoir and **bursts** its accumulated valaar up to the surface — the
//! Dusk's once-a-year lifeline (lore: "underground pools become exposed"). The
//! `Quakes` controller mirrors `Bridges`: a dedicated RNG so the ecology stream
//! is untouched, a per-Vraze schedule, and per-pool active windows. The burst is
//! a one-shot transfer at the moment a pool becomes active; the active window is
//! how long the surface fracture stays visible.

use crate::field::Field;
use crate::rng::Rng;
use crate::season::{Calendar, Season, CRAWS_PER_ARH};
use crate::space::{Coord, Layer, Space};

/// One deep reservoir: the surface-plane `(x, y)` columns above its underground
/// pool. A quake transfers the valaar under these columns up to the surface.
#[derive(Clone, Debug, PartialEq)]
pub struct ReservoirPool {
    pub cells: Vec<(u32, u32)>,
}

/// Tunable dials for earthquake scheduling and the burst.
#[derive(Clone, Copy, Debug)]
pub struct QuakeConfig {
    /// Probability a given pool erupts in any given Vraze.
    pub erupt_fraction: f32,
    /// Fraction of each pool cell's *underground* valaar dumped up to the surface
    /// on eruption (the rest stays below).
    pub release_fraction: f32,
    /// Surface fracture stays visible this many craws (range, clamped to Vraze).
    pub min_duration: u32,
    pub max_duration: u32,
}

impl Default for QuakeConfig {
    fn default() -> Self {
        QuakeConfig {
            erupt_fraction: 0.6,
            release_fraction: 0.8,
            min_duration: 20,
            max_duration: 40,
        }
    }
}

/// Cells whose state changed this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuakeUpdate {
    /// Surface-plane `(x, y)` of pools that *erupted* this tick (burst applied).
    pub erupted: Vec<(u32, u32)>,
}

/// Runtime controller: owns the pools, their active state, the current Vraze's
/// per-pool schedule, and a **dedicated RNG**.
pub struct Quakes {
    pools: Vec<ReservoirPool>,
    cfg: QuakeConfig,
    active: Vec<bool>,
    /// `(erupt_craw, end_craw)` within the year for the scheduled Vraze, per pool;
    /// `None` = this pool does not erupt this Vraze.
    window: Vec<Option<(u32, u32)>>,
    scheduled_year: Option<u32>,
    rng: Rng,
}

impl Quakes {
    pub fn new(pools: Vec<ReservoirPool>, cfg: QuakeConfig, seed: u64) -> Self {
        let n = pools.len();
        Quakes {
            pools,
            cfg,
            active: vec![false; n],
            window: vec![None; n],
            scheduled_year: None,
            rng: Rng::new(seed),
        }
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    /// Surface-plane cells of every currently-fracturing pool (viewer overlay).
    pub fn active_cells(&self) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        for (i, p) in self.pools.iter().enumerate() {
            if self.active[i] {
                out.extend_from_slice(&p.cells);
            }
        }
        out
    }

    /// Roll a fresh schedule for the current Vraze: each pool erupts with prob
    /// `erupt_fraction`, at a random offset into the Vraze arh, for a random
    /// fracture duration (clamped inside the arh).
    fn schedule(&mut self) {
        let cfg = self.cfg;
        let vraze_start = Season::Vraze.index() as u32 * CRAWS_PER_ARH;
        let vraze_end = vraze_start + CRAWS_PER_ARH;
        for i in 0..self.window.len() {
            if self.rng.next_unit() < cfg.erupt_fraction {
                let span = (cfg.max_duration.saturating_sub(cfg.min_duration) + 1) as f32;
                let dur = (cfg.min_duration + (self.rng.next_unit() * span) as u32)
                    .clamp(1, CRAWS_PER_ARH);
                let latest_start = CRAWS_PER_ARH.saturating_sub(dur);
                let off =
                    ((self.rng.next_unit() * (latest_start + 1) as f32) as u32).min(latest_start);
                let erupt_craw = vraze_start + off;
                let end_craw = (erupt_craw + dur).min(vraze_end);
                self.window[i] = Some((erupt_craw, end_craw));
            } else {
                self.window[i] = None;
            }
        }
    }

    /// Advance one tick. Rolls a new schedule at the first Vraze tick of each
    /// year; flips each pool active/inactive by the current craw; on the tick a
    /// pool becomes active, **bursts** `release_fraction` of its underground
    /// valaar up to the surface. Returns the cells that erupted this tick.
    pub fn update<S: Space>(
        &mut self,
        calendar: &Calendar,
        space: &S,
        valaar: &mut Field,
    ) -> QuakeUpdate {
        let mut upd = QuakeUpdate::default();
        let in_vraze = calendar.season() == Season::Vraze;
        if in_vraze && self.scheduled_year != Some(calendar.year()) {
            self.scheduled_year = Some(calendar.year());
            self.schedule();
        }
        let craw = calendar.craw();
        for i in 0..self.pools.len() {
            let want_active =
                in_vraze && matches!(self.window[i], Some((o, c)) if craw >= o && craw < c);
            if want_active && !self.active[i] {
                self.active[i] = true;
                for &(x, y) in &self.pools[i].cells {
                    let si = space.index(Coord::new(x, y, Layer::Surface));
                    let ui = space.index(Coord::new(x, y, Layer::Underground));
                    let amt = valaar.get(ui) * self.cfg.release_fraction;
                    if amt > 0.0 {
                        valaar.add(ui, -amt);
                        valaar.add(si, amt);
                    }
                }
                upd.erupted.extend_from_slice(&self.pools[i].cells);
            } else if !want_active && self.active[i] {
                self.active[i] = false;
            }
        }
        upd
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib quakes && cargo clippy --all-targets`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/quakes.rs src/lib.rs
git commit -m "feat: Quakes controller — Vraze earthquakes burst reservoir valaar to the surface"
```

---

### Task 4: Solid reservoir pools in `mapsim`

Turn the scattered reservoir columns into one solid `ReservoirPool` per region and register their cells as Underground sources.

**Files:**
- Modify: `src/mapsim.rs`

**Interfaces:**
- Consumes: `quakes::ReservoirPool`.
- Produces: `pub fn place_reservoir_pools(sw: u32, sh: u32, labels: &[Option<u32>]) -> Vec<ReservoirPool>`; `add_underground_reservoirs(world, &[ReservoirPool])`.

- [ ] **Step 1: Write the failing tests**

Replace the plan-6 reservoir tests in `src/mapsim.rs` (`reservoirs_sit_at_major_sw_tip_and_southmost_island`, `reservoir_placement_is_seed_deterministic` if present, `single_continent_has_no_island_reservoir`, `reservoirs_spread_across_the_south_not_one_blob`, `reservoir_density_increases_toward_the_south`, `major_continent_reservoirs_fill_the_south_west_quadrant`, `island_reservoirs_spread_with_a_dense_south_tip`) with pool-based tests. Use the existing `grid`/`labels` test helpers in the file (mirror how the old tests built `labels`). Concretely:

```rust
    #[test]
    fn two_pools_one_per_southern_region() {
        // A wide continent + a smaller southern island (reuse the existing
        // label-building helper used by the old reservoir tests).
        let (w, h, labels) = two_continent_labels();
        let pools = place_reservoir_pools(w, h, &labels);
        assert_eq!(pools.len(), 2, "major SW quadrant + southmost island");
        assert!(pools.iter().all(|p| !p.cells.is_empty()));
    }

    #[test]
    fn a_pool_is_solid_not_strided() {
        let (w, h, labels) = two_continent_labels();
        let pools = place_reservoir_pools(w, h, &labels);
        // The major pool fills every land cell of its band — adjacent rows share
        // columns (no stride gaps): some column appears in >= 2 different rows.
        let major = &pools[0];
        let mut by_col: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for &(x, _y) in &major.cells {
            *by_col.entry(x).or_insert(0) += 1;
        }
        assert!(by_col.values().any(|&n| n >= 2), "a solid pool stacks rows in a column");
    }

    #[test]
    fn placement_is_deterministic() {
        let (w, h, labels) = two_continent_labels();
        assert_eq!(place_reservoir_pools(w, h, &labels), place_reservoir_pools(w, h, &labels));
    }

    #[test]
    fn single_continent_has_no_island_pool() {
        let (w, h, labels) = single_continent_labels();
        let pools = place_reservoir_pools(w, h, &labels);
        assert_eq!(pools.len(), 1, "only the major continent's pool");
    }
```

> Implementer note: reuse whatever helper the deleted tests used to build `labels` (e.g. a `grid(&[...])` + `label_continents`). If those tests built labels inline, extract a small `two_continent_labels()` / `single_continent_labels()` helper into the test module so all four tests share it. Keep the exact map shapes the plan-6 tests used so the SW-tip/island geometry still holds.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib mapsim`
Expected: FAIL (`cannot find function place_reservoir_pools`).

- [ ] **Step 3: Add the import and the solid-fill helper**

At the top of `src/mapsim.rs`, add to the imports:

```rust
use crate::quakes::ReservoirPool;
```

Add a solid-region helper next to the (now-removed) `gradient_columns`. It reuses the same bounding-box + band + west-half geometry but keeps **every** cell:

```rust
/// Every land cell of continent `target`'s **southern band**, filled solidly (no
/// stride) — one reservoir pool footprint. `west_only` confines it to the
/// continent's western half (the major continent's south-west quadrant). Pure,
/// no RNG.
fn region_cells(sw: u32, sh: u32, labels: &[Option<u32>], target: u32, west_only: bool) -> Vec<(u32, u32)> {
    let idx = |x: u32, y: u32| (y * sw + x) as usize;
    let (mut x_min, mut x_max) = (u32::MAX, 0u32);
    let (mut y_min, mut y_max) = (u32::MAX, 0u32);
    let mut any = false;
    for y in 0..sh {
        for x in 0..sw {
            if labels[idx(x, y)] == Some(target) {
                any = true;
                x_min = x_min.min(x);
                x_max = x_max.max(x);
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }
        }
    }
    let mut cells = Vec::new();
    if !any {
        return cells;
    }
    let span = (y_max - y_min) as f32;
    let y_top = y_max - (span * RESERVOIR_BAND_FRAC).round() as u32;
    let x_limit = if west_only { x_min + (x_max - x_min) / 2 } else { x_max };
    for y in y_top..=y_max {
        for x in x_min..=x_limit {
            if labels[idx(x, y)] == Some(target) {
                cells.push((x, y));
            }
        }
    }
    cells
}
```

Keep the `RESERVOIR_BAND_FRAC` const. Delete `RESERVOIR_MAX_STRIDE`, `RESERVOIR_MIN_STRIDE`, the `gradient_columns` function, the `south_tip` function, and the old `place_underground_reservoirs` function (the pool fill makes the explicit south-tip guarantee redundant — the solid band already includes the southern tip).

- [ ] **Step 4: Add `place_reservoir_pools` and the pool-based `add_underground_reservoirs`**

```rust
/// Place the underground reservoir **pools**: one solid pool over the major
/// (largest) continent's south-west quadrant and one over the southmost other
/// island. Same anchors as plan 6, now contiguous. Pure function of the labels —
/// deterministic, no RNG.
pub fn place_reservoir_pools(sw: u32, sh: u32, labels: &[Option<u32>]) -> Vec<ReservoirPool> {
    let idx = |x: u32, y: u32| (y * sw + x) as usize;
    let n = match labels.iter().flatten().max() {
        Some(&m) => m + 1,
        None => return Vec::new(),
    };
    let mut count = vec![0usize; n as usize];
    let mut south_y = vec![0u32; n as usize];
    for y in 0..sh {
        for x in 0..sw {
            if let Some(l) = labels[idx(x, y)] {
                count[l as usize] += 1;
                south_y[l as usize] = south_y[l as usize].max(y);
            }
        }
    }
    let major = (0..n as usize).max_by_key(|&l| count[l]).unwrap() as u32;
    let mut island: Option<u32> = None;
    let mut best_south = -1i64;
    for l in 0..n {
        if l == major {
            continue;
        }
        if south_y[l as usize] as i64 > best_south {
            best_south = south_y[l as usize] as i64;
            island = Some(l);
        }
    }
    let mut pools = Vec::new();
    let major_cells = region_cells(sw, sh, labels, major, true);
    if !major_cells.is_empty() {
        pools.push(ReservoirPool { cells: major_cells });
    }
    if let Some(isl) = island {
        let isl_cells = region_cells(sw, sh, labels, isl, false);
        if !isl_cells.is_empty() {
            pools.push(ReservoirPool { cells: isl_cells });
        }
    }
    pools
}

/// Register every pool cell as an Underground valaar **source** (slow injection —
/// the thawing sunken crystal) so each pool refills between quakes. No access
/// points / no descendable: valaar stays below until a quake releases it.
pub fn add_underground_reservoirs(world: &mut World<Grid2p5D>, pools: &[ReservoirPool]) {
    for pool in pools {
        for &(x, y) in &pool.cells {
            world.add_source(Coord::new(x, y, Layer::Underground));
        }
    }
}
```

Replace the Task-1 `add_underground_reservoirs(world, cols: &[(u32, u32)])` with this `&[ReservoirPool]` version. Update the `add_reservoirs_registers_underground_sources` test to pass a pool:

```rust
    #[test]
    fn add_reservoirs_registers_underground_sources() {
        let mut world = World::new(Grid2p5D::new(4, 4), Params::default());
        add_underground_reservoirs(&mut world, &[ReservoirPool { cells: vec![(1, 1)] }]);
        assert_eq!(world.sources(), &[Coord::new(1, 1, Layer::Underground)]);
        assert!(world.access_points().is_empty());
    }
```

- [ ] **Step 5: Run tests + clippy**

Run: `cargo test --lib mapsim && cargo clippy --all-targets`
Expected: PASS. (The `mapview.rs` binary won't compile yet — it still calls `place_underground_reservoirs`; that's fixed in Task 6. `cargo test --lib` builds only the library, so it stays green here. Do **not** run a full `cargo test` until Task 6.)

- [ ] **Step 6: Commit**

```bash
git add src/mapsim.rs
git commit -m "feat: reservoirs become solid underground pools (ReservoirPool)"
```

---

### Task 5: Wire `Quakes` into `Sim`

**Files:**
- Modify: `src/sim.rs`

**Interfaces:**
- Consumes: `quakes::Quakes`, `quakes::ReservoirPool`.
- Produces: `Sim::set_quakes(&mut self, Quakes)`; `Sim::quake_active_cells(&self) -> Vec<(u32, u32)>`. `Sim::step` drives the quake burst before `absorb`.

- [ ] **Step 1: Write the failing test**

Add to `src/sim.rs` `tests`:

```rust
    #[test]
    fn a_quake_pulses_surface_valaar_in_vraze() {
        use crate::quakes::{QuakeConfig, Quakes, ReservoirPool};
        use crate::space::{Coord, Layer};

        // 1x1 surface+underground world; a reservoir pool under (0,0).
        let mut world = World::new(Grid2p5D::new(1, 1), Params::default());
        let ui = world.space.index(Coord::new(0, 0, Layer::Underground));
        let si = world.space.index(Coord::new(0, 0, Layer::Surface));
        world.add_source(Coord::new(0, 0, Layer::Underground)); // pool refills below
        let mut sim = Sim::new(world, EcoParams::default(), 1);

        let cfg = QuakeConfig { erupt_fraction: 1.0, release_fraction: 1.0, ..QuakeConfig::default() };
        sim.set_quakes(Quakes::new(vec![ReservoirPool { cells: vec![(0, 0)] }], cfg, 99));

        // Run until we have stepped through a Vraze; the surface cell must have
        // received a burst at some point (it is 0 outside the access mechanic).
        let mut max_surface = 0.0f32;
        for _ in 0..CRAWS_PER_YEAR {
            sim.step();
            max_surface = max_surface.max(sim.world.valaar.get(si));
        }
        assert!(max_surface > 0.0, "a Vraze quake pulsed valaar to the surface");
        let _ = ui; // underground accumulates between quakes
    }
```

(Add `use crate::season::CRAWS_PER_YEAR;` to the test module imports if not already in scope — `season::CRAWS_PER_YEAR` is public.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib sim`
Expected: FAIL (`no method named set_quakes`).

- [ ] **Step 3: Add the field, setter, getter, and step wiring**

In `src/sim.rs`:

Add the import:

```rust
use crate::quakes::Quakes;
```

Add the field to `struct Sim<S>` (next to `bridges`):

```rust
    /// Optional Vraze earthquakes that burst reservoir valaar up (terrain path
    /// only; `None` headless).
    quakes: Option<Quakes>,
```

Initialise it `quakes: None` in `with_climate` (next to `bridges: None`).

Add the setter + getter (next to `set_bridges`):

```rust
    /// Attach earthquakes (call once, after construction).
    pub fn set_quakes(&mut self, quakes: Quakes) {
        self.quakes = Some(quakes);
    }

    /// Surface cells of currently-fracturing pools (viewer overlay); empty if no
    /// quakes are attached.
    pub fn quake_active_cells(&self) -> Vec<(u32, u32)> {
        self.quakes.as_ref().map(|q| q.active_cells()).unwrap_or_default()
    }
```

In `step`, after the land-bridge block and **before** `ecology::absorb`, drive the quakes so the pulse is on the surface before organisms feed:

```rust
        // Vraze earthquakes: burst exposed reservoir valaar up to the surface.
        if let Some(quakes) = self.quakes.as_mut() {
            quakes.update(&self.calendar, &self.world.space, &mut self.world.valaar);
        }

```

- [ ] **Step 4: Run tests + clippy**

Run: `cargo test --lib sim && cargo clippy --all-targets`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/sim.rs
git commit -m "feat: Sim drives Vraze quakes — surface valaar pulse before feeding"
```

---

### Task 6: Viewer — build the quakes, draw the fracture, drop the dig slider

The viewer is not unit-tested in this repo; this task gates on `cargo clippy` + a **manual run** observing a Vraze quake pulse. It also makes the binary compile again (it still references the removed `place_underground_reservoirs`).

**Files:**
- Modify: `src/bin/mapview.rs`

**Interfaces:**
- Consumes: `mapsim::place_reservoir_pools`, `mapsim::add_underground_reservoirs`, `quakes::{QuakeConfig, Quakes}`, `Sim::set_quakes`, `Sim::quake_active_cells`.

- [ ] **Step 1: Fix imports and the feature flag**

In the `use alife::mapsim::{…}` block, remove `place_underground_reservoirs` and keep `add_underground_reservoirs`; add `place_reservoir_pools`. Add:

```rust
use alife::quakes::{QuakeConfig, Quakes};
```

Rename the feature flag and its comment:

```rust
// Feature flag: install the underground valaar reservoirs (the crystalline sink)
// + the Vraze earthquakes that release them. Flip to `false` to build the world
// without reservoirs or quakes.
const ENABLE_RESERVOIRS_AND_QUAKES: bool = true;
```

Add a dedicated quake RNG seed next to `BRIDGE_SEED`:

```rust
const QUAKE_SEED: u64 = 0x9DA4E;
```

- [ ] **Step 2: Add `QuakeConfig` to `Tunables` and build pools + quakes in `build_sim`**

Add a field to `struct Tunables`:

```rust
    quakes: QuakeConfig,
```

Initialise it in `impl Default for Tunables`:

```rust
            quakes: QuakeConfig::default(),
```

Replace the reservoir block in `build_sim` (the `if ENABLE_UNDERGROUND_RESERVOIRS { … }`) with pool placement + quake construction:

```rust
    let mut sim = Sim::new(world, tun.eco, SIM_SEED);
    if ENABLE_RESERVOIRS_AND_QUAKES {
        let pools = place_reservoir_pools(sw, sh, continents);
        add_underground_reservoirs(&mut sim.world, &pools);
        sim.set_quakes(Quakes::new(pools, tun.quakes, QUAKE_SEED));
    }
```

> Note the ordering change: `Sim::new` is now constructed first (it takes `world` by value), then pools are added to `sim.world` and the same `pools` vector seeds `Quakes`. Remove the old `let reservoirs = …; add_underground_reservoirs(&mut world, &reservoirs);` lines that ran before `Sim::new`.

- [ ] **Step 3: Remove the `dig_drain` slider, add quake sliders**

In the Parameters panel, delete the line `slider_f32(ui, "dig_drain", &mut e.dig_drain, 0.0..=1.0);`. In the valaar/bridge "applies on Reload" group (where `BridgeConfig` sliders live), add quake sliders bound to `self.tunables.quakes`:

```rust
            slider_f32(ui, "quake erupt_fraction", &mut self.tunables.quakes.erupt_fraction, 0.0..=1.0);
            slider_f32(ui, "quake release_fraction", &mut self.tunables.quakes.release_fraction, 0.0..=1.0);
```

> Place these beside the existing bridge sliders so they read as "Reload to apply" dials (they feed `build_sim`). Match the exact `&mut` target the surrounding bridge sliders use (some bind to a working `Tunables` copy named `e`/`self.tunables` — follow the local pattern).

- [ ] **Step 4: Add `draw_quakes` and call it on the Surface view**

Add a draw helper that mirrors `draw_bridges`'s projection exactly — quake cells are already sim-resolution `(x, y)`, so (unlike `draw_bridges`, which starts from a flat index `i`) there is no `% sw` / `/ sw`; everything else is identical:

```rust
/// Paint a warm fracture/burst tint over cells of currently-erupting reservoir
/// pools (Surface view only) — the earthquake's visual.
fn draw_quakes(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim) {
    let cell_px = cam.zoom * SIM_SCALE as f32;
    let fracture = egui::Color32::from_rgba_unmultiplied(255, 120, 40, 110);
    for (qx, qy) in t.sim.quake_active_cells() {
        let cx = qx * SIM_SCALE;
        let cy = qy * SIM_SCALE;
        let sx = rect.min.x + (cx as f32 - cam.cx) * cam.zoom;
        let sy = rect.min.y + (cy as f32 - cam.cy) * cam.zoom;
        let cell = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(cell_px, cell_px));
        if rect.intersects(cell) {
            painter.rect_filled(cell, 0.0, fracture);
        }
    }
}
```

Call it in the paint path, on the Surface view, after `draw_valaar`/`draw_bridges` and before/after `draw_organisms` (so the tint sits under the organisms):

```rust
                if t.view_layer == Layer::Surface {
                    draw_quakes(&painter, rect, &self.cam, t);
                }
```

- [ ] **Step 5: Build, lint, and run to verify**

Run: `cargo clippy --all-targets && cargo build --bin mapview`
Expected: clean.

Then a manual check:

Run: `cargo run --bin mapview`
Expected: the sim runs; flipping to the **underground view** shows the two solid reservoir pools as valaar (south-west quadrant + southmost island) accumulating; during **Vraze** a warm fracture tint appears over a pool on the surface and the surface valaar overlay shows a pulse there. No `dig_drain` slider remains; two `quake …` sliders appear (apply on Reload).

- [ ] **Step 6: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: viewer builds quakes + draws the Vraze fracture burst; drop dig slider"
```

---

### Task 7: Docs — status + lore note

**Files:**
- Modify: `README.md`, `~/dev/ideas/world-design.md`

- [ ] **Step 1: `README.md` status table**

Edit the plan-6 row to reflect the repurpose, and add a P1 row. Replace the plan-6 line and add below it:

```markdown
| 6 | Subterranean refuge — underground valaar reservoirs (the crystalline sink) | ✅ done |
| 6b | Quakes & reservoir pools — the `dig` gene is removed; reservoirs are solid underground pools that **Vraze earthquakes** burst up to the surface (the Dusk's annual valaar pulse) | ✅ done |
```

(Adjust the plan-6 wording to drop the "dig gene + underground-view flip" claim, since the dig gene is gone.)

- [ ] **Step 2: `~/dev/ideas/world-design.md` lore note**

Near the "southern sinks / crystalline sink" paragraph, add one line noting the access mechanic is now quakes, not digging:

```markdown
> **Access (resolved 2026-06-23):** the southern sinks are reached not by burrowing
> but by **Vraze earthquakes**, which fracture the crust and burst a reservoir's
> accumulated valaar up to the surface — an annual lifeline for the Dusk. (The
> earlier `dig` gene was removed in favour of this canon mechanic.)
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: status + lore for quakes & reservoir pools (dig removed)"
```

(The `world-design.md` file lives outside the repo; edit it but it is not part of the git commit.)

---

## Self-Review checklist (run before handing off to execution)

- **Spec coverage:** dig removal (Tasks 1–2), solid pools (Task 4), `Quakes` controller + burst (Task 3), sim wiring (Task 5), viewer visual (Task 6), docs (Task 7). ✅
- **Determinism:** `Quakes` uses `QUAKE_SEED` / a dedicated `Rng`; the ecology `Rng` stream is unchanged except for the genome-width shrink (expected). ✅
- **No new deps; `std`-only core; `quakes.rs` imports only `field`/`rng`/`season`/`space`.** ✅
- **Type consistency:** `place_reservoir_pools -> Vec<ReservoirPool>`; `add_underground_reservoirs(world, &[ReservoirPool])`; `Quakes::new(Vec<ReservoirPool>, QuakeConfig, u64)`; `Quakes::update(&Calendar, &S, &mut Field) -> QuakeUpdate`; `Sim::quake_active_cells() -> Vec<(u32, u32)>`. ✅
- **Out of scope (P2):** heat/water offset-feeding and the `valaar_reliance` gene — a separate plan. ✅

## Out of scope / follow-ups (P2 and beyond)

- **P2 — multi-energy:** heat/water as **offset-upkeep** food (cap so they can't pay a surplus → valaar stays primary), gated by a plain `valaar_reliance` gene (genome 9 → 10) with a generalist trade-off; emergent spatial sorting (specialists in the valaar core, generalists in the Dusk) via the valaar gradient × the trade-off — even though climate is spatially uniform.
- **Later:** history-dependent / Lamarckian mutation of `valaar_reliance`; the deferred **crystal-eating** trait (hard to inherit, so Rasconne crystal isn't trivially exploited).
