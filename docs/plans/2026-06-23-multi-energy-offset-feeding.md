# Multi-Energy: Offset-Feeding on Heat/Water + `valaar_reliance` (P2 of the Dusk-survival redesign)

> **For agentic workers:** REQUIRED SUB-SKILL: execute this plan task-by-task with TDD (RED → GREEN → commit per task). Steps use checkbox (`- [ ]`) syntax for tracking. (This repo's owner executes plans **inline, single-agent** — not via subagent dispatch.)

**Goal:** Break valaar's monopoly as the one food: let organisms offset part of their upkeep by harvesting ambient **heat** and **water**, gated by three new genes (`valaar_reliance`, `heat_affinity`, `water_affinity`), so life can persist in the valaar-poor Dusk and, over generations, evolve *off* valaar — while valaar stays the only path to a reproductive surplus.

**Architecture:** An **offset-upkeep** model. Valaar uptake (`absorb`) is scaled by `valaar_reliance` so reliant specialists extract field valaar best. A new `substitute_feed` step grants each surface organism a small per-tick energy **relief**, capped at `(1 − valaar_reliance) · basal_cost` and at what it can actually harvest (`substitute_rate · (heat·heat_affinity + water·water_affinity)`). Relief is *added* before `metabolize` subtracts the full basal cost, so the net is a reduced upkeep that can never exceed zero (substitutes keep you alive, never grow you). The seed population starts at `valaar_reliance = 1.0`, `heat_affinity = water_affinity = 0.0`, so behaviour is identical to P1 until those genes mutate.

**Tech Stack:** Rust (edition 2021); `std`-only engine core; `eframe`/`egui` for the viewer. **No new crates.**

**Depends on:** P1 (`docs/plans/2026-06-23-quakes-and-reservoir-pools.md`) — shipped. Genome is currently **9** traits (`size, valaar_efficiency, speed, diet, repro_threshold, lifespan, heat_tolerance, drought_tolerance, swim`).

## Global Constraints

- **Language:** Rust, edition 2021. **No new dependencies.** Engine core stays `std`-only (`genome`, `organism`, `ecology`, `params`, `sim`, `mapsim`). Viewer code lives only in `src/bin/mapview.rs`.
- **Implicit selection only:** nothing scores fitness. Generalists spread (or not) because low-reliance organisms that offset upkeep from heat/water survive the valaar-poor Dusk and reproduce — never because a term rewards it.
- **Valaar stays primary by construction:** relief is capped at the basal cost share, so substitutes never produce a surplus; only valaar uptake feeds the energy bank toward the reproduction threshold.
- **Determinism:** identical seed + params ⇒ identical run. No new `Rng` draws are added in the hot loop (the new genes ride the existing per-birth `mutate`). Adding 3 genome traits changes the genome array width, so `Genome::random`/`mutate` consume more draws and runs differ from pre-P2 runs — **expected**.
- **Backward-compatible seed:** the seeded population starts `valaar_reliance = 1.0`, `heat_affinity = 0.0`, `water_affinity = 0.0` — so a fresh run behaves exactly like P1 until mutation explores the new axes.
- **Formatting:** do **not** run `cargo fmt`. Hand-format; gate on `cargo clippy --all-targets`.
- **Version control:** commit per task on `main`. Plain commit messages, **no `Co-Authored-By` trailer**.
- **Visual-rep convention:** the reliance/affinity mechanic ships with a viewer visual (Task 5) — part of "done".

---

## Design assumptions (flagged for veto)

- **D1 — three new genes appended at indices 9/10/11:** `valaar_reliance` (9), `heat_affinity` (10), `water_affinity` (11). `TRAIT_COUNT 9 → 12`.
- **D2 — the trade-off is in valaar uptake:** `absorb` gains `× valaar_reliance`. Reliance 1.0 = today's behaviour; lower reliance = weaker valaar extraction (the cost of being a generalist). Predation is unchanged (it eats organisms, not the field).
- **D3 — offset, not income:** substitutes *reduce* effective upkeep via a capped relief added before `metabolize`. `relief = min((1 − valaar_reliance) · basal_cost, substitute_rate · (heat·heat_affinity + water·water_affinity))`. A reliance-1.0 organism gets zero relief; a fully-generalist (reliance 0) organism can offset its whole basal cost if it harvests enough.
- **D4 — surface only:** `substitute_feed` skips `Layer::Underground` (nothing lives there post-P1; heat/water are surface climate). Mirrors `environmental_stress`.
- **D5 — `substitute_rate` is a new `EcoParams` dial**, default `0.05` (a runnable placeholder, tuned later), exposed as a live viewer slider.
- **D6 — seed genome:** `seed_on_fed_land` (and `bin/life.rs`) seed `valaar_reliance = 1.0`, `heat_affinity = 0.0`, `water_affinity = 0.0`. Mutation perturbs them like any trait (the *history-dependent / Lamarckian* mutation of reliance is **out of scope** — a later plan).
- **D7 — visual:** a viewer toggle to **colour organisms by `valaar_reliance`** (valaar-blue at 1.0 → ochre/earth at 0.0), so generalists are visibly distinct as they fill the Dusk. Off by default (diet colouring stays the default).

---

## File Structure

- `src/genome.rs` — **modified**: 3 new traits, `TRAIT_COUNT 12`, `from_array`/`to_array`, a test.
- (compile-driven) every `Genome::from_array([…])` literal grows 9 → 12 by appending `1.0, 0.0, 0.0` (reliance 1, no affinity) — preserves existing behaviour. Sites in `src/genome.rs`, `src/organism.rs`, `src/ecology.rs`, `src/mapsim.rs`, `src/population.rs`, `src/bin/life.rs`, `tests/ecology.rs`.
- `src/params.rs` — **modified**: `substitute_rate` dial + default.
- `src/ecology.rs` — **modified**: `absorb` scales by `valaar_reliance`; new `substitute_feed`.
- `src/sim.rs` — **modified**: call `substitute_feed` before `metabolize`.
- `src/mapsim.rs` — **modified**: `seed_on_fed_land` seeds the new genes (already touches the seed literal in Task 1).
- `src/bin/mapview.rs` — **modified**: `substitute_rate` slider; "colour by reliance" toggle + tint.
- `README.md` — **modified**: status row.

---

### Task 1: Three new genes (genome 9 → 12)

**Files:**
- Modify: `src/genome.rs`
- Modify (compile-driven): every `Genome::from_array([…])` literal — `src/organism.rs`, `src/ecology.rs`, `src/mapsim.rs`, `src/population.rs`, `src/bin/life.rs`, `tests/ecology.rs`.

**Interfaces:**
- Produces: `Genome.valaar_reliance` (idx 9), `Genome.heat_affinity` (idx 10), `Genome.water_affinity` (idx 11); `pub const TRAIT_COUNT: usize = 12`.

- [ ] **Step 1: Write the failing test** — in `src/genome.rs` `tests`:

```rust
    #[test]
    fn new_energy_genes_round_trip_at_the_tail() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.5, 0.25, 0.75]);
        assert_eq!(g.valaar_reliance, 0.5);
        assert_eq!(g.heat_affinity, 0.25);
        assert_eq!(g.water_affinity, 0.75);
        assert_eq!(TRAIT_COUNT, 12);
        assert_eq!(g.to_array()[9], 0.5);
        assert_eq!(g.to_array()[11], 0.75);
    }
```

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib genome` → expected array length mismatch / field-not-found.

- [ ] **Step 3: Add the fields, bump the count, extend the arrays**

In `src/genome.rs`: set `pub const TRAIT_COUNT: usize = 12;`. After the `swim` field add:

```rust
    /// Reliance on valaar as a food: 1.0 = fully valaar-dependent (today's
    /// behaviour); lower lets a larger share of upkeep be met from substitutes,
    /// at the cost of weaker valaar extraction.
    pub valaar_reliance: f32,
    /// How well the organism harvests ambient heat to offset upkeep (thermotrophy).
    pub heat_affinity: f32,
    /// How well the organism harvests ambient water to offset upkeep (osmotrophy).
    pub water_affinity: f32,
```

Extend `from_array` (after `swim: a[8],`):

```rust
            valaar_reliance: a[9],
            heat_affinity: a[10],
            water_affinity: a[11],
```

Extend `to_array` (after `self.swim,`):

```rust
            self.valaar_reliance,
            self.heat_affinity,
            self.water_affinity,
```

Lengthen the existing genome unit-test literals (`round_trips_through_array`, `clamped_pins_to_unit_interval`, `mutate_stays_in_bounds_even_from_extremes`, `trait_count_is_nine_and_swim_is_last` → rename to `_is_twelve` and assert `TRAIT_COUNT == 12`) by appending three values.

- [ ] **Step 4: Run the build to list every stale literal** — `cargo build 2>&1 | head -40`. Each stale `from_array([…])` reports `expected an array with a fixed size of 12 elements, found one with 9`.

- [ ] **Step 5: Grow every stale literal 9 → 12 by appending `1.0, 0.0, 0.0`**

This sets `valaar_reliance = 1.0`, `heat_affinity = 0.0`, `water_affinity = 0.0` — preserving every test's existing behaviour (valaar uptake unchanged at `×1.0`, no substitute relief). Apply to all sites in `src/organism.rs`, `src/ecology.rs`, `src/mapsim.rs`, `src/population.rs`, `src/bin/life.rs`, `tests/ecology.rs`. **The seed literal in `mapsim::seed_on_fed_land` and `bin/life.rs` must also use `1.0, 0.0, 0.0`** (D6) — confirm those two are the seed genomes.

- [ ] **Step 6: Run tests + clippy** — `cargo test && cargo clippy --all-targets`. Expected: PASS (all existing tests unchanged in behaviour).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add valaar_reliance + heat/water affinity genes (genome 12 traits)"
```

---

### Task 2: `valaar_reliance` scales valaar uptake

**Files:** Modify: `src/ecology.rs` (`absorb` + a test).

**Interfaces:** Consumes `Genome.valaar_reliance`. Changes `absorb`'s gain formula.

- [ ] **Step 1: Write the failing test** — in `src/ecology.rs` `tests`:

```rust
    #[test]
    fn low_reliance_absorbs_less_valaar() {
        let space = Grid2p5D::new(1, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        field.set(0, 1.0);
        // Same genome but reliance 1.0 vs 0.5 (autotroph, full efficiency).
        let mut full = Population::new();
        full.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5, 1.0, 0.0, 0.0]),
            Coord::new(0, 0, Layer::Surface),
            0.0,
        ));
        let mut half = Population::new();
        half.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0, 0.0]),
            Coord::new(0, 0, Layer::Surface),
            0.0,
        ));
        let mut f1 = field.clone();
        let mut f2 = field.clone();
        absorb(&space, &mut f1, &mut full, &eco);
        absorb(&space, &mut f2, &mut half, &eco);
        let g_full = full.organisms()[0].energy;
        let g_half = half.organisms()[0].energy;
        assert!(g_full > 0.0 && g_half > 0.0);
        assert!((g_half - g_full * 0.5).abs() < 1e-6, "reliance 0.5 absorbs half as much");
    }
```

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib ecology::tests::low_reliance_absorbs_less_valaar` → FAIL (full == half today).

- [ ] **Step 3: Scale the gain by reliance** — in `absorb`, change the `want` line:

```rust
        let want = eco.uptake_rate * o.genome.valaar_efficiency * o.genome.valaar_reliance * auto * avail;
```

- [ ] **Step 4: Run tests** — `cargo test --lib ecology && cargo clippy --all-targets`. Expected: PASS (existing absorb tests use reliance 1.0, unchanged).

- [ ] **Step 5: Commit**

```bash
git add src/ecology.rs
git commit -m "feat: valaar_reliance scales valaar uptake (the generalist trade-off)"
```

---

### Task 3: `substitute_feed` — heat/water offset upkeep

**Files:** Modify: `src/params.rs` (`substitute_rate`), `src/ecology.rs` (`substitute_feed` + test).

**Interfaces:** Produces `pub fn substitute_feed<S: Space>(space: &S, heat: &Field, water: &Field, pop: &mut Population, eco: &EcoParams)`. Adds `EcoParams::substitute_rate`.

- [ ] **Step 1: Add the param** — in `src/params.rs`, add a field (next to `valaar_drain`):

```rust
    /// Energy harvested per unit of (heat·heat_affinity + water·water_affinity)
    /// to offset upkeep. Scales the substitute relief.
    pub substitute_rate: f32,
```

and in `Default`: `substitute_rate: 0.05,`.

- [ ] **Step 2: Write the failing test** — in `src/ecology.rs` `tests`:

```rust
    #[test]
    fn substitutes_offset_upkeep_only_for_generalists() {
        let space = Grid2p5D::new(1, 1);
        let eco = EcoParams::default();
        let mut heat = crate::field::Field::zeros(space.len());
        let mut water = crate::field::Field::zeros(space.len());
        heat.set(0, 1.0); // a hot, dry cell
        let at = Coord::new(0, 0, Layer::Surface);
        // A heat-adapted generalist (reliance 0.0, heat_affinity 1.0) gains relief.
        let mut gen = Population::new();
        gen.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5, 0.0, 1.0, 0.0]),
            at, 1.0,
        ));
        // A pure valaar specialist (reliance 1.0) gains nothing.
        let mut spec = Population::new();
        spec.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 0.0]),
            at, 1.0,
        ));
        let mut water2 = water.clone();
        substitute_feed(&space, &heat, &mut water, &mut gen, &eco);
        substitute_feed(&space, &heat, &mut water2, &mut spec, &eco);
        assert!(gen.organisms()[0].energy > 1.0, "generalist offsets upkeep from heat");
        assert_eq!(spec.organisms()[0].energy, 1.0, "specialist gets no relief");
        // Relief never exceeds basal cost (no surplus from substitutes).
        let basal = {
            use crate::organism::Organism;
            gen.organisms()[0].basal_cost(&eco)
        };
        assert!(gen.organisms()[0].energy - 1.0 <= basal + 1e-6, "relief capped at basal cost");
    }
```

> Note `substitute_feed` takes `water: &Field` (read-only); the test passes `&mut water`/`&water` — adjust the signature in Step 3 to `&Field` and the call to `&water`. (Heat/water are never mutated by feeding.)

- [ ] **Step 3: Run to verify it fails** — `cargo test --lib ecology::tests::substitutes_offset_upkeep_only_for_generalists` → FAIL (`substitute_feed` not found).

- [ ] **Step 4: Implement `substitute_feed`** — in `src/ecology.rs` (near `environmental_stress`):

```rust
/// Grant each **surface** organism a per-tick energy *relief* from harvesting
/// ambient heat/water, capped at `(1 - valaar_reliance) · basal_cost` (so a
/// fully-reliant organism gets none) and at what it can harvest
/// (`substitute_rate · (heat·heat_affinity + water·water_affinity)`). Added before
/// `metabolize` subtracts the full basal cost, so substitutes reduce upkeep but
/// never produce a surplus — valaar stays the only path to growth.
pub fn substitute_feed<S: Space>(
    space: &S,
    heat: &Field,
    water: &Field,
    pop: &mut Population,
    eco: &EcoParams,
) {
    for o in pop.organisms_mut() {
        if o.pos.layer == Layer::Underground {
            continue;
        }
        let i = space.index(o.pos);
        let fuel = eco.substitute_rate
            * (heat.get(i) * o.genome.heat_affinity + water.get(i) * o.genome.water_affinity);
        let cap = (1.0 - o.genome.valaar_reliance).max(0.0) * o.basal_cost(eco);
        let relief = fuel.min(cap).max(0.0);
        o.energy += relief;
    }
}
```

(Add `use crate::organism::Organism;` at the top of `ecology.rs` if `basal_cost` isn't already in scope — `metabolize` already calls `o.basal_cost(eco)`, so it is.)

- [ ] **Step 5: Run tests + clippy** — `cargo test --lib ecology && cargo clippy --all-targets`. Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/params.rs src/ecology.rs
git commit -m "feat: substitute_feed — heat/water offset upkeep for low-reliance generalists"
```

---

### Task 4: Wire `substitute_feed` into the tick

**Files:** Modify: `src/sim.rs` (`step` + a test).

**Interfaces:** Consumes `ecology::substitute_feed`.

- [ ] **Step 1: Write the failing test** — in `src/sim.rs` `tests`:

```rust
    #[test]
    fn a_generalist_outlasts_a_specialist_in_a_warm_valaar_desert() {
        use crate::space::{Coord, Layer};
        // No valaar anywhere, but a hot surface: only a heat-generalist offsets upkeep.
        let world = World::new(Grid2p5D::new(1, 1), Params::default());
        let mut sim = Sim::new(world, EcoParams::default(), 1);
        for i in 0..sim.heat.len() {
            sim.heat.set(i, 1.0);
        }
        let at = Coord::new(0, 0, Layer::Surface);
        // index 9/10/11 = reliance / heat_affinity / water_affinity
        let generalist = Genome::from_array([0.0, 1.0, 0.0, 0.0, 0.9, 1.0, 1.0, 0.5, 0.5, 0.0, 1.0, 0.0]);
        let specialist = Genome::from_array([0.0, 1.0, 0.0, 0.0, 0.9, 1.0, 1.0, 0.5, 0.5, 1.0, 1.0, 0.0]);
        sim.seed_organism(TraitOrganism::new(generalist, at, 0.2));
        sim.seed_organism(TraitOrganism::new(specialist, at, 0.2));
        let energy = |s: &Sim<Grid2p5D>, idx: usize| s.pop.organisms()[idx].energy;
        // The generalist (idx 0) should retain more energy than the specialist (idx 1)
        // after a few ticks of upkeep with no valaar income.
        for _ in 0..3 {
            sim.step();
        }
        assert!(
            energy(&sim, 0) > energy(&sim, 1),
            "heat-generalist offsets upkeep; specialist does not"
        );
    }
```

> Climate relaxes `heat` toward the season target each `step`; seeding `heat = 1.0` plus Rasgun/early seasons keeps it warm enough across 3 ticks for the relief to bite. If the margin is fragile, assert over a single `ecology::substitute_feed` + `metabolize` pair instead — but prefer the integration test.

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib sim::tests::a_generalist_outlasts_a_specialist_in_a_warm_valaar_desert` → FAIL (no offset wired in).

- [ ] **Step 3: Wire it in** — in `Sim::step`, between `environmental_stress` and `metabolize`:

```rust
        ecology::environmental_stress(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::substitute_feed(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
```

- [ ] **Step 4: Run tests + clippy** — `cargo test && cargo clippy --all-targets`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/sim.rs
git commit -m "feat: Sim grants substitute relief each tick before metabolism"
```

---

### Task 5: Viewer — `substitute_rate` slider + colour-by-reliance

The viewer is not unit-tested; gate on `cargo clippy --all-targets` + a manual run.

**Files:** Modify: `src/bin/mapview.rs`.

- [ ] **Step 1: Live `substitute_rate` slider** — in `parameters_ui`, in the "Ecology (live)" group (next to `valaar_drain`):

```rust
        slider_f32(ui, "substitute_rate", &mut e.substitute_rate, 0.0..=0.5);
```

- [ ] **Step 2: A colour-by-reliance toggle** — add a `bool` field `show_reliance` to `TileSim` (default `false`), a checkbox in the stats panel (next to the `Valaar`/`Charts` checkboxes):

```rust
        ui.checkbox(&mut t.show_reliance, "Colour by reliance");
```

(Thread `show_reliance: false` through every `TileSim { … }` construction.)

- [ ] **Step 3: Tint organisms by reliance when the toggle is on** — `draw_organisms` takes the flag; when set, the fill is a lerp from ochre (reliance 0 — generalist) to valaar-blue (reliance 1 — specialist) instead of the diet colour:

```rust
        // inside draw_organisms' per-organism loop, replacing the diet fill when show_reliance:
        let fill = if show_reliance {
            let r = o.genome.valaar_reliance.clamp(0.0, 1.0);
            // ochre (179,134,52) -> valaar-blue (70,120,210)
            let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * r) as u8;
            egui::Color32::from_rgb(lerp(179, 70), lerp(134, 120), lerp(52, 210))
        } else {
            /* existing diet-based fill */
        };
```

Pass `t.show_reliance` into `draw_organisms` at its call site.

- [ ] **Step 4: Build, lint, run** — `cargo clippy --all-targets && cargo build --bin mapview`, then `cargo run --bin mapview`: a `substitute_rate` slider appears; toggling "Colour by reliance" recolours organisms (uniform blue at first — everyone starts reliance 1.0 — drifting toward ochre in valaar-poor regions over many years).

- [ ] **Step 5: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: viewer substitute_rate slider + colour-by-reliance overlay"
```

---

### Task 6: Docs

**Files:** Modify: `README.md`.

- [ ] **Step 1: Status row** — after the `6b` row:

```markdown
| 7 | Multi-energy — heat/water offset upkeep (thermotrophy/osmotrophy) gated by `valaar_reliance` + `heat_affinity`/`water_affinity`; valaar stays the only path to a surplus | ✅ done |
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: status row for multi-energy offset-feeding"
```

---

## Self-Review checklist

- **Coverage:** genes (T1), valaar trade-off (T2), substitute offset + param (T3), tick wiring (T4), viewer slider + visual (T5), docs (T6). ✅
- **Valaar primary:** relief capped at basal cost ⇒ no surplus from substitutes. ✅
- **Backward-compatible seed:** reliance 1.0 / affinities 0.0 ⇒ a fresh run matches P1 until mutation. ✅
- **Type consistency:** `substitute_feed(space, heat, water, pop, eco)`; `EcoParams::substitute_rate`; genome indices 9/10/11. ✅
- **Out of scope (later):** history-dependent / Lamarckian mutation of `valaar_reliance`; crystal-eating trait; spatially-varying climate (geothermal vents, monsoon zones).
