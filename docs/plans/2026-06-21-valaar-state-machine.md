# Plan 5b — Valaar State-Machine (seasonal phases + crystal deposits) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make valaar change *phase* with the season — not just abundance — so that Miscre's gaseous valaar spreads to the Dusk, Vraze's crystalline valaar **freezes into a persistent `crystal` field that carries valaar forward in time** past decay, and Dansch/Laisp drain harder.

**Architecture:** A pure `ValaarPhase` (Liquid / Gaseous / Crystalline / Sparse) is derived from the season; each phase carries `PhaseDynamics` dials (diffusion passes, decay multiplier, freeze/thaw rates). `World` gains a second `crystal: Field` that never diffuses or decays. `World::step` runs `diffuse_passes` diffusion passes per tick (stable spread without breaking the `< 0.25` coefficient limit). `Sim::step` sets the per-phase `diffuse_passes` and `decay` each tick (the way it already sets `source_rate`), then calls `valaar::freeze_thaw` to move valaar in/out of `crystal`. Organisms only ever read `world.valaar`, so the ecology RNG stream is byte-for-byte unchanged — the *environment* evolves differently by design, but no new random draws are introduced.

**Tech Stack:** Rust (edition 2021). No new dependencies. Applies everywhere a valaar field exists (headless `life`/`sim` and the `mapview` viewer alike). Visualising `crystal` in the viewer is **out of scope** (deferred to plan 5c).

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** `~/dev/ideas/world-design.md` §F ("Seasons — the 6-arh cycle": Rasgun liquid/surge → Goscon stable → Miscre gaseous/fog (airborne valaar reaches the Dusk) → Vraze crystalline → Dansch/Laisp sparse) and the plan-5b deferral in `~/dev/alife/docs/plans/2026-06-20-land-bridges-and-mutation-field.md`.

**Builds on (shipped — exact interfaces consumed):**

- `field::Field` — `zeros(len) -> Field`, `len() -> usize`, `get(i) -> f32`, `set(i, v)`, `add(i, dv)`, `total() -> f32`.
- `space::{Coord, Layer, Space, Grid2p5D}` — `Grid2p5D::new(w, h)`, `Space::{len, index, in_bounds}`; `Coord::new(x, y, Layer::Surface)`.
- `season::{Season, Calendar}` — variants in order `Rasgun, Goscon, Miscre, Vraze, Dansch, Laisp`; `Calendar::{season() -> Season, advance()}`.
- `world::{World<S>, Params}` — `Params { source_rate, diffuse_rate, layer_exchange, decay }` (Default `1.0, 0.2, 0.1, 0.01`); `World { pub space, pub valaar: Field, pub params: Params, .. }`; `World::new(space, params)`; `World::step()` runs `inject_sources → diffuse_planar → exchange_layers → decay`.
- `valaar::{inject_sources, diffuse_planar, exchange_layers, decay}` — pure functions over `(Space, Field)`; `diffuse_planar(&space, &mut field, rate)`.
- `climate::{Climate, SeasonTarget, target}` — `target(&climate, season) -> SeasonTarget`; `SeasonTarget { heat, water, valaar_mult }`.
- `sim::Sim<S>` — pub `world`/`pop`/`eco`/`rng`/`calendar`/`heat`/`water`/`climate`; private `base_source: f32` (captured at construction); `with_climate` builds the only struct literal, `new` delegates to it; `step()` sets `world.params.source_rate = base_source * target.valaar_mult` then calls `world.step()`.

## Global Constraints

- **Language:** Rust, edition 2021. No new dependencies.
- **Formatting / lint gate (repo convention):** repo is **not** rustfmt-clean — **do NOT run `cargo fmt`**. Hand-format to match surrounding code (4-space indent, `//!`/`///` docs). Gate every task on `cargo test` and `cargo clippy --all-targets` (clippy must be clean — warnings fail the gate).
- **Determinism:** no wall-clock, no new RNG draws. All phase logic (`for_season`, `dynamics`, `freeze_thaw`, the diffusion loop) is pure and calendar-driven. The ecology RNG stream is untouched; the valaar *environment* legitimately evolves differently than before (that is the feature).
- **Numerical stability:** the planar diffusion coefficient must stay `< 0.25`. Gaseous "spreads further" is achieved by running **more diffusion passes** at the stable coefficient, never by raising the coefficient.
- **Layering:** logic lives in lib modules with unit tests; no binary changes in this plan.
- **Version control:** work on `main` (per user). Commit per task, plain message, **no `Co-Authored-By` trailer**. `git add` only the files each task names. The working tree also carries unrelated user tuning edits in `params.rs`/`bridges.rs` — leave them; do not stage them.

---

## Design assumptions (flagged for veto)

- **A1 — Phase mapping.** Rasgun, Goscon → `Liquid`; Miscre → `Gaseous`; Vraze → `Crystalline`; Dansch, Laisp → `Sparse`.
- **A2 — Per-phase dynamics dials** (fixed constants in `ValaarPhase::dynamics`, tunable in one place; not per-`Sim` overridable — YAGNI):

  | Phase | `diffuse_passes` | `decay_mult` | `freeze_rate` | `thaw_rate` |
  |-------|------------------|--------------|---------------|-------------|
  | Liquid | 1 | 1.0 | 0.0 | 0.02 |
  | Gaseous | 3 | 1.0 | 0.0 | 0.02 |
  | Crystalline | 0 | 0.5 | 0.10 | 0.0 |
  | Sparse | 1 | 1.5 | 0.0 | 0.02 |

- **A3 — Crystal is locked storage.** `crystal` never diffuses or decays. Crystalline phase freezes `freeze_rate` of each cell's valaar into crystal; every other phase thaws `thaw_rate` of each cell's crystal back to valaar. `valaar + crystal` total is conserved by `freeze_thaw`.
- **A4 — Source unchanged by phase.** Per-season abundance still rides the existing `climate` `valaar_mult` (source). Phase only governs spread (diffusion passes), drain (decay), and crystallisation.
- **A5 — Organisms see only `valaar`.** No ecology change: thawed crystal re-enters the valaar field and is absorbed normally. Crystal is invisible to organisms except through the valaar it releases.
- **A6 — Crystal applies to all cells** (surface + underground), since valaar exists on both layers.

---

## File Structure

- `src/valaar.rs` — **modified**: add `ValaarPhase`, `PhaseDynamics`, `ValaarPhase::{for_season, dynamics}`, `freeze_thaw`, and `use crate::season::Season;`; tests.
- `src/world.rs` — **modified**: `crystal: Field` field + init; `diffuse_passes` in `Params` + `Default`; `World::step` diffusion loop; tests.
- `src/sim.rs` — **modified**: `base_decay` field; phase wiring in `step`; a new `#[cfg(test)] mod tests`.
- `README.md` — **modified**: note the seasonal valaar phases + crystal.

---

### Task 1: `ValaarPhase` + per-phase dynamics

**Files:**
- Modify: `src/valaar.rs` (enum, `PhaseDynamics`, `for_season`, `dynamics`, import; tests)

**Interfaces:**
- Produces: `ValaarPhase { Liquid, Gaseous, Crystalline, Sparse }` (Copy, Eq); `ValaarPhase::for_season(Season) -> ValaarPhase`; `ValaarPhase::dynamics(self) -> PhaseDynamics`; `PhaseDynamics { diffuse_passes: u32, decay_mult: f32, freeze_rate: f32, thaw_rate: f32 }` (Copy).

- [ ] **Step 1: Add the `Season` import**

In `src/valaar.rs`, extend the imports at the top (currently `use crate::field::Field;` / `use crate::space::{Coord, Layer, Space};`):

```rust
use crate::field::Field;
use crate::season::Season;
use crate::space::{Coord, Layer, Space};
```

- [ ] **Step 2: Write the failing tests**

Add to the `tests` module in `src/valaar.rs` (it already has `use super::*;`):

```rust
    #[test]
    fn phase_maps_each_season() {
        use ValaarPhase::*;
        assert_eq!(ValaarPhase::for_season(Season::Rasgun), Liquid);
        assert_eq!(ValaarPhase::for_season(Season::Goscon), Liquid);
        assert_eq!(ValaarPhase::for_season(Season::Miscre), Gaseous);
        assert_eq!(ValaarPhase::for_season(Season::Vraze), Crystalline);
        assert_eq!(ValaarPhase::for_season(Season::Dansch), Sparse);
        assert_eq!(ValaarPhase::for_season(Season::Laisp), Sparse);
    }

    #[test]
    fn phase_dynamics_match_their_intent() {
        let c = ValaarPhase::Crystalline.dynamics();
        assert_eq!(c.diffuse_passes, 0, "crystalline locks valaar in place");
        assert!(c.freeze_rate > 0.0 && c.thaw_rate == 0.0, "crystalline freezes, never thaws");
        let g = ValaarPhase::Gaseous.dynamics();
        assert!(
            g.diffuse_passes > ValaarPhase::Liquid.dynamics().diffuse_passes,
            "gaseous spreads further than liquid"
        );
        let s = ValaarPhase::Sparse.dynamics();
        assert!(s.decay_mult > 1.0, "sparse drains faster");
        assert!(ValaarPhase::Liquid.dynamics().thaw_rate > 0.0, "non-crystalline thaws crystal back");
    }
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib valaar`
Expected: FAIL — `cannot find type ValaarPhase` (does not compile).

- [ ] **Step 4: Implement the phase type**

In `src/valaar.rs`, add after the imports and before the first `pub fn`:

```rust
/// The physical *phase* valaar takes in a given season. Beyond abundance (the
/// climate `valaar_mult` on the source), the phase changes valaar's *dynamics*:
/// how far it spreads, how fast it drains, and whether it crystallises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValaarPhase {
    /// Rasgun / Goscon — flows and diffuses normally.
    Liquid,
    /// Miscre — gaseous/fog: spreads much further (airborne valaar reaches the Dusk).
    Gaseous,
    /// Vraze — crystalline: stops spreading and freezes a fraction of valaar into
    /// solid `crystal` that resists decay until later seasons thaw it.
    Crystalline,
    /// Dansch / Laisp — sparse: valaar drains faster and the Dusk bites hardest.
    Sparse,
}

/// Per-phase dynamics dials.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseDynamics {
    /// Diffusion passes per tick (0 = locked). Multiple *stable* passes spread
    /// valaar further without exceeding the per-pass coefficient limit (< 0.25).
    pub diffuse_passes: u32,
    /// Multiplier on the base decay rate this phase.
    pub decay_mult: f32,
    /// Fraction of each cell's valaar that freezes into crystal per tick.
    pub freeze_rate: f32,
    /// Fraction of each cell's crystal that thaws back into valaar per tick.
    pub thaw_rate: f32,
}

impl ValaarPhase {
    /// The phase valaar takes in `season`.
    pub fn for_season(season: Season) -> Self {
        match season {
            Season::Rasgun | Season::Goscon => ValaarPhase::Liquid,
            Season::Miscre => ValaarPhase::Gaseous,
            Season::Vraze => ValaarPhase::Crystalline,
            Season::Dansch | Season::Laisp => ValaarPhase::Sparse,
        }
    }

    /// The tuning dials for this phase (tune the whole state-machine here).
    pub fn dynamics(self) -> PhaseDynamics {
        match self {
            ValaarPhase::Liquid => PhaseDynamics {
                diffuse_passes: 1,
                decay_mult: 1.0,
                freeze_rate: 0.0,
                thaw_rate: 0.02,
            },
            ValaarPhase::Gaseous => PhaseDynamics {
                diffuse_passes: 3,
                decay_mult: 1.0,
                freeze_rate: 0.0,
                thaw_rate: 0.02,
            },
            ValaarPhase::Crystalline => PhaseDynamics {
                diffuse_passes: 0,
                decay_mult: 0.5,
                freeze_rate: 0.10,
                thaw_rate: 0.0,
            },
            ValaarPhase::Sparse => PhaseDynamics {
                diffuse_passes: 1,
                decay_mult: 1.5,
                freeze_rate: 0.0,
                thaw_rate: 0.02,
            },
        }
    }
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib valaar && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add src/valaar.rs
git commit -m "feat: ValaarPhase + per-phase dynamics dials"
```

---

### Task 2: `crystal` field on `World`

**Files:**
- Modify: `src/world.rs` (`crystal` field + init; tests)

**Interfaces:**
- Produces: `World.crystal: Field` — public, sized to `space.len()`, zero at construction.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/world.rs`:

```rust
    #[test]
    fn crystal_defaults_to_empty() {
        let space = Grid2p5D::new(2, 2);
        let world = World::new(space, Params::default());
        assert_eq!(world.crystal.len(), world.space.len());
        assert_eq!(world.crystal.total(), 0.0);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib world`
Expected: FAIL — `no field crystal on type World`.

- [ ] **Step 3: Add the field**

In `src/world.rs`, add to `struct World<S: Space>` immediately after `pub valaar: Field,`:

```rust
    pub valaar: Field,
    /// Frozen valaar (the Vraze crystalline phase). Never diffuses or decays;
    /// `valaar::freeze_thaw` moves valaar in and out of it. Sized to `space.len()`.
    pub crystal: Field,
```

In `World::new`, after `let valaar = Field::zeros(space.len());`, add `let crystal` and set it in the struct literal:

```rust
        let valaar = Field::zeros(space.len());
        let crystal = Field::zeros(space.len());
        World {
            space,
            valaar,
            crystal,
            params,
            sources: Vec::new(),
            access_points: Vec::new(),
            passability: None,
            swimmable: None,
        }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib world && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/world.rs
git commit -m "feat: World gains a locked crystal field"
```

---

### Task 3: `diffuse_passes` in `Params` + `World::step` loop

**Files:**
- Modify: `src/world.rs` (`Params` field + `Default`; `step` loop; tests)

**Interfaces:**
- Produces: `Params.diffuse_passes: u32` (Default `1`); `World::step` runs `diffuse_passes` planar diffusion passes per tick (`0` = no spread).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/world.rs`:

```rust
    #[test]
    fn diffuse_passes_controls_spread() {
        // One hot centre cell on a 1-D strip, isolated from sources/decay/exchange.
        let remaining = |passes: u32| {
            let mut p = Params::default();
            p.diffuse_passes = passes;
            p.source_rate = 0.0;
            p.decay = 0.0;
            let mut w = World::new(Grid2p5D::new(5, 1), p);
            let c = w.space.index(Coord::new(2, 0, Layer::Surface));
            w.valaar.set(c, 1.0);
            w.step();
            w.valaar.get(c)
        };
        assert_eq!(remaining(0), 1.0, "no passes: nothing leaves the centre");
        assert!(remaining(1) < remaining(0), "one pass spreads some out");
        assert!(remaining(3) < remaining(1), "more passes spread more out");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib world`
Expected: FAIL — `no field diffuse_passes on type Params`.

- [ ] **Step 3: Add the dial**

In `src/world.rs`, add to `struct Params` immediately after `pub diffuse_rate: f32,`:

```rust
    pub diffuse_rate: f32,
    /// Planar diffusion passes per tick (0 = locked). More stable passes spread
    /// valaar further without raising the per-pass coefficient past its limit.
    pub diffuse_passes: u32,
```

In the `Default for Params` impl, after `diffuse_rate: 0.2,`:

```rust
            diffuse_rate: 0.2,
            diffuse_passes: 1,
```

- [ ] **Step 4: Loop the diffusion in `step`**

In `src/world.rs` `World::step`, replace the single diffusion call

```rust
        valaar::diffuse_planar(&self.space, &mut self.valaar, self.params.diffuse_rate);
```

with

```rust
        for _ in 0..self.params.diffuse_passes {
            valaar::diffuse_planar(&self.space, &mut self.valaar, self.params.diffuse_rate);
        }
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib world && cargo clippy --all-targets`
Expected: PASS, clippy clean. (Existing world/valaar tests still pass: `diffuse_passes` defaults to `1`, the previous behaviour.)

- [ ] **Step 6: Commit**

```bash
git add src/world.rs
git commit -m "feat: phase-controlled diffusion passes per tick"
```

---

### Task 4: `valaar::freeze_thaw`

**Files:**
- Modify: `src/valaar.rs` (function + tests)

**Interfaces:**
- Consumes: `ValaarPhase`, `PhaseDynamics` (Task 1), `field::Field`.
- Produces: `freeze_thaw(valaar: &mut Field, crystal: &mut Field, phase: ValaarPhase, d: &PhaseDynamics)` — Crystalline freezes `valaar → crystal` by `freeze_rate`; every other phase thaws `crystal → valaar` by `thaw_rate`. Conserves `valaar + crystal` total.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/valaar.rs`:

```rust
    #[test]
    fn freeze_moves_valaar_into_crystal_and_conserves() {
        let mut v = Field::zeros(2);
        let mut c = Field::zeros(2);
        v.set(0, 1.0);
        let d = ValaarPhase::Crystalline.dynamics(); // freeze_rate 0.10
        let before = v.total() + c.total();
        freeze_thaw(&mut v, &mut c, ValaarPhase::Crystalline, &d);
        assert!((c.get(0) - 0.10).abs() < 1e-6);
        assert!((v.get(0) - 0.90).abs() < 1e-6);
        assert!((v.total() + c.total() - before).abs() < 1e-6, "valaar+crystal conserved");
    }

    #[test]
    fn thaw_returns_crystal_to_valaar_off_crystalline() {
        let mut v = Field::zeros(1);
        let mut c = Field::zeros(1);
        c.set(0, 1.0);
        let d = ValaarPhase::Liquid.dynamics(); // thaw_rate 0.02
        freeze_thaw(&mut v, &mut c, ValaarPhase::Liquid, &d);
        assert!((c.get(0) - 0.98).abs() < 1e-6);
        assert!((v.get(0) - 0.02).abs() < 1e-6);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib valaar`
Expected: FAIL — `cannot find function freeze_thaw`.

- [ ] **Step 3: Implement `freeze_thaw`**

In `src/valaar.rs`, add after the `impl ValaarPhase { .. }` block (before the `tests` module):

```rust
/// Move valaar between the liquid field and the frozen `crystal` field. During
/// the `Crystalline` phase a `freeze_rate` fraction of each cell's valaar freezes
/// into crystal; in every other phase a `thaw_rate` fraction of crystal thaws
/// back into valaar. Crystal never diffuses or decays, so it carries valaar
/// forward in time. Conserves the `valaar + crystal` total.
pub fn freeze_thaw(valaar: &mut Field, crystal: &mut Field, phase: ValaarPhase, d: &PhaseDynamics) {
    match phase {
        ValaarPhase::Crystalline => {
            for i in 0..valaar.len() {
                let f = valaar.get(i) * d.freeze_rate;
                valaar.set(i, valaar.get(i) - f);
                crystal.add(i, f);
            }
        }
        _ => {
            for i in 0..crystal.len() {
                let t = crystal.get(i) * d.thaw_rate;
                crystal.set(i, crystal.get(i) - t);
                valaar.add(i, t);
            }
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib valaar && cargo clippy --all-targets`
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/valaar.rs
git commit -m "feat: valaar::freeze_thaw — crystal accumulates in Vraze, thaws after"
```

---

### Task 5: Drive the phase from `Sim::step`

**Files:**
- Modify: `src/sim.rs` (`base_decay` field, phase wiring in `step`, new `tests` module)

**Interfaces:**
- Consumes: `ValaarPhase::{for_season, dynamics}`, `valaar::freeze_thaw` (Tasks 1/4), `Params.diffuse_passes` (Task 3), `World.crystal` (Task 2).
- Produces: each tick, valaar's phase sets `world.params.diffuse_passes` + `world.params.decay`, and `crystal` is updated. No public API change beyond the existing `step`.

- [ ] **Step 1: Add the import + captured base decay**

In `src/sim.rs`, add to the imports:

```rust
use crate::valaar::{self, ValaarPhase};
```

Add a field to `struct Sim<S: Space>` immediately after `base_source: f32,`:

```rust
    base_source: f32,
    /// Un-multiplied decay, captured at construction so the per-phase multiplier
    /// always scales the same base.
    base_decay: f32,
```

In `with_climate`, capture it next to `base_source` and set it in the struct literal:

```rust
        let base_source = world.params.source_rate;
        let base_decay = world.params.decay;
```

```rust
            base_source,
            base_decay,
```

- [ ] **Step 2: Wire the phase into `step`**

In `src/sim.rs` `step`, replace the existing season/source block

```rust
        let season = self.calendar.season();

        // [A6] valaar abundance breathes with the season.
        self.world.params.source_rate =
            self.base_source * climate::target(&self.climate, season).valaar_mult;

        self.world.step();
```

with

```rust
        let season = self.calendar.season();
        let phase = ValaarPhase::for_season(season);
        let dynamics = phase.dynamics();

        // [A6] valaar abundance breathes with the season; its *phase* sets how
        // far it spreads (diffusion passes), how fast it drains (decay), and
        // whether it crystallises.
        let target = climate::target(&self.climate, season);
        self.world.params.source_rate = self.base_source * target.valaar_mult;
        self.world.params.diffuse_passes = dynamics.diffuse_passes;
        self.world.params.decay = self.base_decay * dynamics.decay_mult;

        self.world.step();
        valaar::freeze_thaw(&mut self.world.valaar, &mut self.world.crystal, phase, &dynamics);
```

(The `season` binding is still used later in `step` for the mutation spike and bridges; leave the rest of the function unchanged.)

- [ ] **Step 3: Write the failing integration test**

`src/sim.rs` has no `tests` module yet — add one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Grid2p5D, Space};
    use crate::world::{Params, World};

    #[test]
    fn crystal_builds_in_vraze_and_thaws_after() {
        let mut world = World::new(Grid2p5D::new(3, 3), Params::default());
        for i in 0..world.space.len() {
            world.valaar.set(i, 1.0); // give freezing something to grab everywhere
        }
        let mut sim = Sim::new(world, EcoParams::default(), 1);

        while sim.calendar.season() != Season::Vraze {
            sim.step();
        }
        let entering = sim.world.crystal.total();
        for _ in 0..30 {
            sim.step();
        }
        let in_vraze = sim.world.crystal.total();
        assert!(in_vraze > entering, "crystal accumulates during Vraze");

        while sim.calendar.season() == Season::Vraze {
            sim.step();
        }
        for _ in 0..50 {
            sim.step();
        }
        let after = sim.world.crystal.total();
        assert!(after < in_vraze, "crystal thaws once Vraze passes");
    }
}
```

- [ ] **Step 4: Run to verify it fails, then passes**

Run: `cd ~/dev/alife && cargo test --lib sim`
Before Step 1/2 the call sites would not compile; with them in place the test should PASS. If the test is red, the wiring in Step 2 is wrong — re-check before proceeding.

- [ ] **Step 5: Full suite + lint**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets`
Expected: PASS, clippy clean. (Headless `tests/*.rs` still pass — the valaar field evolves differently, but no test asserts exact valaar totals, and no new RNG draws were added.)

- [ ] **Step 6: Commit**

```bash
git add src/sim.rs
git commit -m "feat: drive valaar phase + crystal freeze/thaw from Sim::step"
```

---

### Task 6: Document plan 5b

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Extend the seasons note**

In `README.md`, find the bullet that currently ends "…the mutation rate spikes during **Rasgun**." and append:

```markdown
  Valaar also changes **phase** with the season: gaseous in **Miscre** (fog
  carries it out to the Dusk), **crystalline** in **Vraze** (it freezes into a
  locked *crystal* store that thaws back over later seasons, carrying valaar
  forward in time), and sparse in **Dansch/Laisp**.
```

- [ ] **Step 2: Lint check and commit**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

```bash
git add README.md
git commit -m "docs: note seasonal valaar phases + crystal store"
```

---

## Self-Review

**Spec coverage:**
- Seasonal valaar *phase* (not just abundance) → Task 1 (`ValaarPhase` + `dynamics`). ✓
- Miscre gaseous valaar reaches the Dusk → Task 3 (`diffuse_passes`) + Task 5 (Gaseous = 3 passes). ✓
- Vraze crystalline → persistent deposit carried forward in time → Task 2 (`crystal` field), Task 4 (`freeze_thaw`), Task 5 (driven each tick). ✓
- Dansch/Laisp sparse (harder drain) → Task 1 (Sparse `decay_mult` 1.5) + Task 5 (decay scaled). ✓
- Determinism / no new RNG draws → Global Constraints + pure phase logic; ecology stream untouched (A5). ✓
- Stability (< 0.25 coefficient) → passes-not-coefficient (Task 3, Global Constraints). ✓

**Placeholder scan:** every code step shows full code; run steps give command + expected result; no TBD/TODO. ✓

**Type consistency:** `ValaarPhase` (Copy, Eq) and `PhaseDynamics { diffuse_passes: u32, decay_mult/freeze_rate/thaw_rate: f32 }` defined in Task 1, consumed unchanged by `freeze_thaw` (Task 4) and `Sim::step` (Task 5); `Params.diffuse_passes: u32` (Task 3) set in Task 5; `World.crystal: Field` (Task 2) read in Task 5; `freeze_thaw(&mut Field, &mut Field, ValaarPhase, &PhaseDynamics)` signature matches its Task-5 call. ✓

**Deferred (later plans):** visualising `crystal`/phase in the viewer → **plan 5c** (params dropdown + charts). The `digging`/subterranean gene + underground as a climate refuge → **plan 6**.
