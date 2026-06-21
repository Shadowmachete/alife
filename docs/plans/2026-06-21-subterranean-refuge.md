# Subterranean Refuge (digging gene + underground valaar reservoirs) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give life a reason and a way to go underground — deep valaar reservoirs (the **crystalline sink**: Vraze-frozen valaar that sank under the cold far-south) beneath the **south-west tip of the major continent** and the **southmost island**, plus a `dig` gene that lets an organism **descend into the Underground layer** to shelter from seasonal stress and forage those reservoirs — and make it all visible via an underground-view flip that reuses the plan-5d valaar overlay.

**Architecture:** A 10th genome trait `dig` (parallel to `swim`): genetic threshold `can_dig()`, gated at runtime by a `World.descendable` mask. `ecology::move_organisms` gains a vertical move candidate — Surface↔Underground at a reservoir **access column** — chosen by the same valaar-gradient logic and charged a tunable `dig_drain`. `ecology::environmental_stress` exempts the Underground layer (the refuge). Reservoirs are placed deterministically from continent labels (`mapsim::place_underground_reservoirs`) and wired as underground valaar **sources** + descent **access points**. The viewer gains a layer flip that renders the Underground as a dark cave with the plan-5d overlay showing the reservoirs and the diggers sheltering in them.

**Tech Stack:** Rust (edition 2021); std-only engine core; `eframe`/`egui` for the viewer. No new crates.

**Depends on:** **plan 5d (valaar phase overlay)** — `viewer::draw_valaar` and the colour helpers; the overlay is reused for the underground view. Implement 5d first.

## Global Constraints

- **Language:** Rust, edition 2021. **No new dependencies.**
- **Engine stays std-only:** `genome`, `organism`, `ecology`, `world`, `mapsim` must not pull in `eframe`/`egui`/`serde`. Viewer/egui code lives only in `bin/mapview.rs`.
- **Implicit selection only:** nothing scores fitness. Digging spreads (or not) because diggers that reach the reservoirs survive the harsh seasons and reproduce — never because a fitness term rewards `dig`.
- **Determinism:** identical seed + params ⇒ identical run. Descent is evaluated inside the existing **once-per-organism** `Rng` draw in `move_organisms` (the stream advances exactly as before). Reservoir placement is a **pure function of the map** (no `Rng`). Adding the 10th trait changes genome arrays, so runs differ from pre-plan-6 runs — expected.
- **Valaar is the one currency:** the underground reservoirs are valaar (food); the Underground being stress-free is a *shelter*, not a second currency.
- **Formatting:** do **not** run `cargo fmt` (repo isn't rustfmt-clean). Hand-format; gate on `cargo clippy --all-targets`.
- **Version control:** commit per task. Plain commit messages, **no `Co-Authored-By` trailer**.
- **Visual-rep convention:** the underground-view flip + the diggers rendered below + the reservoirs lit by the plan-5d overlay are this mechanic's visual — part of "done".

---

## Design assumptions (flagged for veto)

- **D1 — `dig` is the 10th trait** (`TRAIT_COUNT 9 → 10`), appended at array index 9. `can_dig()` = `dig > DIG_THRESHOLD` with `DIG_THRESHOLD = 0.5` (mirrors `SWIM_THRESHOLD`).
- **D2 — descent is gated by a `descendable` mask** (a `World` mask like `swimmable`), `true` at both the surface and underground cell of every reservoir access column. A `can_dig` organism standing on a descendable cell may move to the same `(x,y)` on the other layer.
- **D3 — descent is gradient-driven:** the other-layer cell is just another move candidate, taken only if its valaar beats the best surface option — so diggers descend toward the reservoir and resurface when the surface recovers. It costs `move_cost·speed` + `dig_drain`.
- **D4 — the Underground is a climate refuge:** `environmental_stress` applies **zero** heat/drought penalty to organisms on `Layer::Underground`. (Climate is spatially uniform today, so this switch is what makes "below" safe.)
- **D5 — reservoirs are passable underground cells holding field valaar**, not impassable `Valaar` terrain — so diggers can stand in them and `absorb`. They register as underground **sources** (slow injection) and their columns as **access points** (so `exchange_layers` lets some valaar seep up to the Dusk surface, lore-appropriate, and descent is allowed there).
- **D6 — placement:** the **major continent** = the largest flood-fill component. Its reservoir sits at the cell maximising `y + (sw-1-x)` (south-most + west-most → the SW tip). The **southmost island** = the non-major component whose southernmost cell has the largest `y`; its reservoir sits at that component's southernmost (tie → westmost) cell. Each is stamped as a radius-1 cluster.
- **D7 — `dig_drain` default 0.15** (same as `valaar_drain`), a panel slider.

---

## File Structure

- `src/genome.rs` — **modified**: add `dig` (10th trait), bump `TRAIT_COUNT`, extend `from_array`/`to_array`.
- `src/organism.rs` — **modified**: `DIG_THRESHOLD`, `can_dig()`.
- `src/params.rs` — **modified**: `dig_drain` dial + default.
- `src/ecology.rs` — **modified**: `move_organisms` gains a `descendable` param + descent candidate; `environmental_stress` exempts the Underground.
- `src/world.rs` — **modified**: `descendable` mask (`set_descendable`/`descendable`).
- `src/sim.rs` — **modified**: pass `world.descendable()` to `move_organisms`.
- `src/mapsim.rs` — **modified**: `place_underground_reservoirs`, `add_underground_reservoirs`; seed `dig`.
- `src/bin/mapview.rs` — **modified**: wire reservoirs into `build_sim`; `view_layer` flip; layer-aware overlay/organisms; `dig_drain` slider.
- `~/dev/ideas/world-design.md` — **modified**: the crystalline-sink reservoir lore.
- `README.md` — **modified**: status rows for plans 5d + 6.

Many test files carry 9-element `Genome::from_array([...])` literals; Task 1 updates every one.

---

### Task 1: The `dig` gene (10th genome trait)

**Files:**
- Modify: `src/genome.rs` (`TRAIT_COUNT`, `dig` field, `from_array`/`to_array`, a new test)
- Modify (compile-driven): every `Genome::from_array([...])` with a 9-element literal — in `src/genome.rs`, `src/organism.rs`, `src/ecology.rs`, `src/mapsim.rs` tests/helpers.

**Interfaces:**
- Produces: `Genome.dig: f32` at array index 9; `pub const TRAIT_COUNT: usize = 10`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/genome.rs`:

```rust
    #[test]
    fn dig_round_trips_as_the_tenth_trait() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.33]);
        assert_eq!(g.dig, 0.33);
        assert_eq!(g.to_array()[9], 0.33);
        assert_eq!(TRAIT_COUNT, 10);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib genome`
Expected: **compile error** — `Genome` has no field `dig` / array length mismatch.

- [ ] **Step 3: Update the `Genome` core**

In `src/genome.rs`: set `pub const TRAIT_COUNT: usize = 10;`. Add the field after `swim`:

```rust
    /// Capacity to descend into the Underground layer at a reservoir access
    /// column. Genetically "able to dig" when `> 0.5` (see `TraitOrganism::can_dig`).
    pub dig: f32,
```

Extend `from_array` and `to_array` with index 9:

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
            swim: a[8],
            dig: a[9],
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
            self.heat_tolerance,
            self.drought_tolerance,
            self.swim,
            self.dig,
        ]
    }
```

(`random`, `clamped`, `mutate` iterate `TRAIT_COUNT` and need no change.)

- [ ] **Step 4: Fix every 9-element `from_array` literal (compiler-driven)**

Run `cd ~/dev/alife && cargo test --lib 2>&1 | head -50` and let the `expected an array with a fixed size of 10` / `expected 10 elements` errors point you to each site. For each 9-element `[ ... ]` passed to `Genome::from_array`, **append a 10th value** (`0.5` for neutral test genomes; `0.0` where the trait should be off). Known sites:
  - `src/genome.rs` tests: `round_trips_through_array` (use the 10-element array from Step 1's sibling test pattern), `swim_round_trips_as_the_ninth_trait`, `clamped_pins_to_unit_interval`, `mutate_stays_in_bounds_even_from_extremes`, `mutate_with_zero_rate_is_identity`.
  - `src/organism.rs` tests: `genome_with` and `swim_genome` helpers; any inline genome.
  - `src/ecology.rs` tests: `genome`, `swimmer`, `predator`, `prey`, `tol_genome` helpers and every inline `Genome::from_array([...])` in the move/predate/reproduce tests.
  - `src/mapsim.rs`: the `Genome::from_array([...])` inside `seed_on_fed_land` (append `0.0` — dig arises by mutation) and the `g`/inline genomes in its tests (append `0.5`).

- [ ] **Step 5: Run the whole suite**

Run: `cd ~/dev/alife && cargo test`
Expected: all tests pass (the new `dig_round_trips_as_the_tenth_trait` included).

- [ ] **Step 6: Lint + commit**

```bash
cd ~/dev/alife && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: add dig as the 10th genome trait"
```

---

### Task 2: `can_dig` on `TraitOrganism`

**Files:**
- Modify: `src/organism.rs`

**Interfaces:**
- Consumes: `Genome.dig` (Task 1).
- Produces: `pub fn can_dig(&self) -> bool`; `const DIG_THRESHOLD: f32 = 0.5`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/organism.rs` (next to `can_swim_requires_gene_above_threshold`):

```rust
    fn dig_genome(dig: f32) -> Genome {
        Genome::from_array([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, dig])
    }

    #[test]
    fn can_dig_requires_gene_above_threshold() {
        let pos = Coord::new(0, 0, Layer::Surface);
        assert!(TraitOrganism::new(dig_genome(0.9), pos, 1.0).can_dig());
        assert!(!TraitOrganism::new(dig_genome(0.4), pos, 1.0).can_dig());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib organism`
Expected: **compile error** — no method `can_dig`.

- [ ] **Step 3: Implement**

In `src/organism.rs`, add the constant next to `SWIM_THRESHOLD`:

```rust
/// Genetic threshold above which an organism is *able* to dig (descend).
const DIG_THRESHOLD: f32 = 0.5;
```

And the method in `impl TraitOrganism`, next to `can_swim`:

```rust
    /// Whether this organism can descend into the Underground at a reservoir
    /// access column. Purely genetic (Darwinian): able iff `genome.dig > DIG_THRESHOLD`.
    pub fn can_dig(&self) -> bool {
        self.genome.dig > DIG_THRESHOLD
    }
```

- [ ] **Step 4: Run + lint + commit**

```bash
cd ~/dev/alife && cargo test --lib organism && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: TraitOrganism::can_dig (genetic threshold)"
```

---

### Task 3: `dig_drain` ecology dial

**Files:**
- Modify: `src/params.rs`

**Interfaces:**
- Produces: `EcoParams.dig_drain: f32` (default `0.15`).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/params.rs`:

```rust
    #[test]
    fn dig_drain_default_is_present() {
        let eco = EcoParams::default();
        assert!(eco.dig_drain > 0.0);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib params`
Expected: **compile error** — no field `dig_drain`.

- [ ] **Step 3: Implement**

In `src/params.rs`, add the field after `valaar_drain`:

```rust
    /// Energy an organism spends to descend/ascend one layer (the dig cost).
    pub dig_drain: f32,
```

And in `Default`, after `valaar_drain: 0.15,`:

```rust
            dig_drain: 0.15,
```

- [ ] **Step 4: Run the suite + lint + commit**

(`EcoParams` is built by `Default` everywhere, so no other call sites change.)

```bash
cd ~/dev/alife && cargo test && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: dig_drain ecology dial"
```

---

### Task 4: `descendable` mask on `World`

**Files:**
- Modify: `src/world.rs`

**Interfaces:**
- Produces: `pub fn set_descendable(&mut self, mask: Vec<bool>)`; `pub fn descendable(&self) -> Option<&[bool]>`. A new `descendable: Option<Vec<bool>>` field, defaulting to `None`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/world.rs`:

```rust
    #[test]
    fn descendable_defaults_none_and_round_trips() {
        let space = Grid2p5D::new(2, 2);
        let mut world = World::new(space, Params::default());
        assert!(world.descendable().is_none());
        let mut mask = vec![false; world.space.len()];
        mask[0] = true;
        world.set_descendable(mask);
        assert_eq!(world.descendable().unwrap().len(), world.space.len());
        assert!(world.descendable().unwrap()[0]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib world`
Expected: **compile error** — no method `set_descendable`.

- [ ] **Step 3: Implement**

In `src/world.rs`, add the field to the `World` struct (after `swimmable`):

```rust
    /// Per-cell "descendable" mask (sized to `space.len()`): `true` where a
    /// `can_dig` organism may move to the other layer at the same `(x, y)` (a
    /// reservoir access column). `None` = nothing descendable.
    descendable: Option<Vec<bool>>,
```

Initialise it in `World::new` (after `swimmable: None,`):

```rust
            descendable: None,
```

Add the accessors in the first `impl<S: Space> World<S>` block (next to `swimmable`):

```rust
    /// Install a per-cell descendable mask (`true` = a digger may switch layers
    /// here). Length must equal `space.len()` (all layers, `Space::index` order).
    pub fn set_descendable(&mut self, mask: Vec<bool>) {
        debug_assert_eq!(mask.len(), self.space.len(), "mask must cover every cell");
        self.descendable = Some(mask);
    }

    /// The descendable mask, if one was installed.
    pub fn descendable(&self) -> Option<&[bool]> {
        self.descendable.as_deref()
    }
```

- [ ] **Step 4: Run + lint + commit**

```bash
cd ~/dev/alife && cargo test --lib world && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: World descendable mask"
```

---

### Task 5: Descent / ascent in `move_organisms`

**Files:**
- Modify: `src/ecology.rs` (`move_organisms` signature + body; new tests)
- Modify: `src/sim.rs` (pass `world.descendable()`)
- Modify (compile-driven): every direct `move_organisms(...)` call in `src/ecology.rs` tests (add the new `None` arg).

**Interfaces:**
- Consumes: `World.descendable()` (Task 4); `TraitOrganism::can_dig` (Task 2); `EcoParams.dig_drain` (Task 3); `space::Layer`.
- Produces: `move_organisms(space, field, pop, eco, rng, passable, swimmable, descendable)` — one extra trailing `descendable: Option<&[bool]>` param.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/ecology.rs`. (A `1×1` grid has two cells: surface index 0, underground index 1 — no planar neighbours, so only the descent candidate can move the organism.)

```rust
    fn digger(dig: f32) -> Genome {
        // speed 1.0 so it always acts; diet 0 autotroph.
        Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5, dig])
    }

    /// surface(idx0) over a rich underground reservoir(idx1); both descendable.
    fn shaft(under: f32) -> (Grid2p5D, crate::field::Field, Vec<bool>, Vec<bool>) {
        let space = Grid2p5D::new(1, 1);
        let mut field = crate::field::Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Underground)), under);
        let passable = vec![true; space.len()];
        let descendable = vec![true; space.len()];
        (space, field, passable, descendable)
    }

    #[test]
    fn digger_descends_toward_a_rich_reservoir() {
        let (space, field, passable, descendable) = shaft(9.0);
        let eco = EcoParams::default();
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(digger(0.9), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), None, Some(&descendable));
        assert_eq!(pop.organisms()[0].pos, Coord::new(0, 0, Layer::Underground));
        let expected = 5.0 - eco.move_cost - eco.dig_drain;
        assert!((pop.organisms()[0].energy - expected).abs() < 1e-6, "pays move + dig_drain");
    }

    #[test]
    fn non_digger_cannot_descend() {
        let (space, field, passable, descendable) = shaft(9.0);
        let eco = EcoParams::default();
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(digger(0.1), start, 5.0)); // gene below threshold
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), None, Some(&descendable));
        assert_eq!(pop.organisms()[0].pos, start);
    }

    #[test]
    fn digger_stays_without_a_descendable_column() {
        let (space, field, passable, _descendable) = shaft(9.0);
        let eco = EcoParams::default();
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(digger(0.9), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), None, None);
        assert_eq!(pop.organisms()[0].pos, start, "no descendable mask -> no descent");
    }

    #[test]
    fn digger_ascends_when_the_surface_is_richer() {
        let space = Grid2p5D::new(1, 1);
        let mut field = crate::field::Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 9.0); // surface richer
        let passable = vec![true; space.len()];
        let descendable = vec![true; space.len()];
        let eco = EcoParams::default();
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(digger(0.9), Coord::new(0, 0, Layer::Underground), 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), None, Some(&descendable));
        assert_eq!(pop.organisms()[0].pos, Coord::new(0, 0, Layer::Surface));
    }
```

- [ ] **Step 2: Update the existing direct callers in `ecology.rs` tests**

Every existing `move_organisms(...)` call in the `ecology.rs` tests has 7 args ending in the `swimmable` slot. Append `, None` (no descendable) to each so they compile against the new signature. (Sites: `moves_uphill_toward_richer_valaar`, `at_local_max_it_stays_put`, `does_not_step_onto_impassable_richer_neighbor`, `boxed_in_organism_stays_and_pays_nothing`, `tunneller_teleports_across_band_to_far_bank`, `non_tunneller_cannot_cross_the_band`, `tunneller_stays_if_far_bank_is_poorer`, `no_landing_beyond_band_blocks_the_crossing`.)

- [ ] **Step 3: Run to verify the new tests fail**

Run: `cd ~/dev/alife && cargo test --lib ecology`
Expected: **compile error** — `move_organisms` takes 7 args, not 8 (until Step 4).

- [ ] **Step 4: Implement the descent candidate**

In `src/ecology.rs`, add `Layer` to the `space` import:

```rust
use crate::space::{Coord, Layer, Space};
```

Change the `move_organisms` signature to take the trailing param:

```rust
pub fn move_organisms<S: Space>(
    space: &S,
    field: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
    passable: Option<&[bool]>,
    swimmable: Option<&[bool]>,
    descendable: Option<&[bool]>,
) {
```

Inside the per-organism loop, introduce a `best_descend` flag beside `best_width`:

```rust
        let mut best = o.pos;
        let mut best_v = field.get(space.index(o.pos));
        let mut best_width = 0u32; // Valaar cells crossed to reach `best` (0 = a walk)
        let mut best_descend = false; // whether `best` is reached by switching layers
```

After the `if o.can_swim() { ... }` tunnel block, add the descent block:

```rust
        // Diggers can switch layers at a descendable access column, choosing the
        // other-layer cell if it holds more valaar (descend to a reservoir, or
        // resurface when the surface recovers).
        if o.can_dig() {
            let here_descendable = matches!(descendable, Some(m) if m[space.index(o.pos)]);
            if here_descendable {
                let other_layer = match o.pos.layer {
                    Layer::Surface => Layer::Underground,
                    Layer::Underground => Layer::Surface,
                };
                let other = Coord::new(o.pos.x, o.pos.y, other_layer);
                let oi = space.index(other);
                let open = match passable {
                    Some(m) => m[oi],
                    None => true,
                };
                if open {
                    let v = field.get(oi);
                    if v > best_v {
                        best_v = v;
                        best = other;
                        best_width = 0;
                        best_descend = true;
                    }
                }
            }
        }
```

Extend the move-cost block to charge `dig_drain` on a layer switch:

```rust
        if best != o.pos {
            o.pos = best;
            o.energy -= eco.move_cost * o.genome.speed;
            o.energy -= eco.valaar_drain * best_width as f32; // 0 for a plain walk
            if best_descend {
                o.energy -= eco.dig_drain;
            }
        }
```

- [ ] **Step 5: Update `Sim::step`**

In `src/sim.rs`, the `ecology::move_organisms(...)` call passes `self.world.swimmable()` last; add the descendable arg:

```rust
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
            self.world.passability(),
            self.world.swimmable(),
            self.world.descendable(),
        );
```

- [ ] **Step 6: Run the suite**

Run: `cd ~/dev/alife && cargo test`
Expected: all pass, including the four new descent tests.

- [ ] **Step 7: Lint + commit**

```bash
cd ~/dev/alife && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: diggers descend/ascend at reservoir access columns"
```

---

### Task 6: The Underground is a climate refuge

**Files:**
- Modify: `src/ecology.rs` (`environmental_stress`)

**Interfaces:**
- Consumes: `space::Layer` (imported in Task 5).
- Produces: no signature change — `environmental_stress` now exempts `Layer::Underground`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/ecology.rs`:

```rust
    #[test]
    fn underground_shelters_from_heat_and_drought() {
        let space = Grid2p5D::new(1, 1);
        let eco = EcoParams::default();
        let mut heat = crate::field::Field::zeros(space.len());
        let water = crate::field::Field::zeros(space.len());
        // Scorching, bone-dry on BOTH layers (climate is uniform today).
        for i in 0..space.len() {
            heat.set(i, 1.0);
        }
        let surf = Coord::new(0, 0, Layer::Surface);
        let under = Coord::new(0, 0, Layer::Underground);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(tol_genome(0.0, 0.0), surf, 5.0)); // intolerant, on top
        pop.spawn(TraitOrganism::new(tol_genome(0.0, 0.0), under, 5.0)); // intolerant, below
        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert!(pop.organisms()[0].energy < 5.0, "surface life suffers the heat/drought");
        assert_eq!(pop.organisms()[1].energy, 5.0, "the Underground is a refuge");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib ecology::tests::underground_shelters_from_heat_and_drought`
Expected: **FAIL** — the underground organism also loses energy.

- [ ] **Step 3: Implement**

In `src/ecology.rs`, in `environmental_stress`, skip Underground organisms at the top of the loop:

```rust
    for o in pop.organisms_mut() {
        if o.pos.layer == Layer::Underground {
            continue; // the Underground is sheltered from seasonal heat/drought
        }
        let i = space.index(o.pos);
```

- [ ] **Step 4: Run + lint + commit**

```bash
cd ~/dev/alife && cargo test --lib ecology && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: Underground layer shelters from seasonal stress"
```

---

### Task 7: Reservoir placement + wiring (`mapsim`)

**Files:**
- Modify: `src/mapsim.rs` (`place_underground_reservoirs`, `add_underground_reservoirs`, seed `dig`; tests)

**Interfaces:**
- Consumes: `label_continents` output (`labels`); `World`/`Coord`/`Layer`/`Space`.
- Produces:
  - `pub fn place_underground_reservoirs(sw: u32, sh: u32, labels: &[Option<u32>]) -> Vec<(u32, u32)>`
  - `pub fn add_underground_reservoirs(world: &mut World<Grid2p5D>, cols: &[(u32, u32)])`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/mapsim.rs` (the existing `grid` helper maps `'O'`→Ocean, `'V'`→Valaar, else Land):

```rust
    #[test]
    fn reservoirs_sit_at_major_sw_tip_and_southmost_island() {
        // A big 3x3 continent (top-left) and a one-cell island at the bottom-right.
        let (w, h, m) = grid(&[
            "LLLO",
            "LLLO",
            "LLLO",
            "OOOL",
        ]);
        let (labels, _n) = label_continents(&m, w, h);
        let cols = place_underground_reservoirs(w, h, &labels);
        // SW tip of the major continent is around (0,2); island is the (3,3) cell.
        assert!(cols.contains(&(0, 2)), "expected a reservoir at the major SW tip, got {cols:?}");
        assert!(cols.contains(&(3, 3)), "expected a reservoir on the southmost island, got {cols:?}");
    }

    #[test]
    fn placement_is_deterministic_and_dedup_sorted() {
        let (w, h, m) = grid(&["LLLO", "LLLO", "LLLO", "OOOL"]);
        let (labels, _n) = label_continents(&m, w, h);
        let a = place_underground_reservoirs(w, h, &labels);
        let b = place_underground_reservoirs(w, h, &labels);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(a, sorted, "columns are sorted + de-duplicated");
    }

    #[test]
    fn single_continent_has_no_island_reservoir() {
        let (w, h, m) = grid(&["LLL", "LLL"]); // one continent, no island
        let (labels, _n) = label_continents(&m, w, h);
        let cols = place_underground_reservoirs(w, h, &labels);
        assert!(!cols.is_empty(), "the major SW tip still gets a reservoir");
        // Only the SW-tip cluster (around (0,1)); no second far cluster.
        assert!(cols.iter().all(|&(x, y)| x <= 1 && y <= 1), "clustered at the SW tip, got {cols:?}");
    }

    #[test]
    fn add_reservoirs_registers_sources_access_points_and_descendable() {
        let (w, h, m) = grid(&["LL", "LL"]);
        let mut world = world_from_materials(w, h, &m);
        add_underground_reservoirs(&mut world, &[(1, 1)]);
        assert!(world.sources().contains(&Coord::new(1, 1, Layer::Underground)));
        assert!(world.access_points().contains(&(1, 1)));
        let desc = world.descendable().expect("descendable installed");
        assert!(desc[world.space.index(Coord::new(1, 1, Layer::Surface))]);
        assert!(desc[world.space.index(Coord::new(1, 1, Layer::Underground))]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib mapsim`
Expected: **compile error** — `cannot find function place_underground_reservoirs`.

- [ ] **Step 3: Implement**

Add to `src/mapsim.rs` (the file already imports `Coord, Grid2p5D, Layer, Space`, `World`):

```rust
/// Cluster radius (cells) stamped around each reservoir centre. [D6]
const RESERVOIR_RADIUS: i32 = 1;

/// Choose the underground reservoir columns: a cluster at the **south-west tip**
/// of the major (largest) continent and a cluster on the **southmost** other
/// island. Returns surface-plane `(x, y)` columns (sorted, de-duplicated) to
/// register as underground sources + descent access points. Pure function of the
/// continent labels — deterministic, no RNG. [D6]
pub fn place_underground_reservoirs(sw: u32, sh: u32, labels: &[Option<u32>]) -> Vec<(u32, u32)> {
    let idx = |x: u32, y: u32| (y * sw + x) as usize;
    let n = match labels.iter().flatten().max() {
        Some(&m) => m + 1,
        None => return Vec::new(),
    };
    // Continent sizes + southernmost row reached by each.
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
    // The southmost non-major island = the other continent reaching the largest y.
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
    // SW tip of the major (max south + west); southmost cell of the island.
    let mut sw_tip: Option<(u32, u32)> = None;
    let mut sw_score = i64::MIN;
    let mut island_cell: Option<(u32, u32)> = None;
    let mut island_score = i64::MIN;
    for y in 0..sh {
        for x in 0..sw {
            match labels[idx(x, y)] {
                Some(l) if l == major => {
                    let s = y as i64 + (sw as i64 - 1 - x as i64);
                    if s > sw_score {
                        sw_score = s;
                        sw_tip = Some((x, y));
                    }
                }
                Some(l) if Some(l) == island => {
                    let s = y as i64 * sw as i64 - x as i64; // southmost, tie -> westmost
                    if s > island_score {
                        island_score = s;
                        island_cell = Some((x, y));
                    }
                }
                _ => {}
            }
        }
    }
    let mut cols = Vec::new();
    for (cx, cy) in [sw_tip, island_cell].into_iter().flatten() {
        for dy in -RESERVOIR_RADIUS..=RESERVOIR_RADIUS {
            for dx in -RESERVOIR_RADIUS..=RESERVOIR_RADIUS {
                let nx = cx as i64 + dx as i64;
                let ny = cy as i64 + dy as i64;
                if nx >= 0 && ny >= 0 && (nx as u32) < sw && (ny as u32) < sh {
                    cols.push((nx as u32, ny as u32));
                }
            }
        }
    }
    cols.sort_unstable();
    cols.dedup();
    cols
}

/// Register each reservoir column on the Underground layer: a valaar **source**
/// (slow injection — the thawing sunken crystal), an **access point** (so valaar
/// seeps up and diggers may descend), and a **descendable** cell on both layers.
pub fn add_underground_reservoirs(world: &mut World<Grid2p5D>, cols: &[(u32, u32)]) {
    let mut desc = vec![false; world.space.len()];
    for &(x, y) in cols {
        world.add_source(Coord::new(x, y, Layer::Underground));
        world.add_access_point(x, y);
        desc[world.space.index(Coord::new(x, y, Layer::Surface))] = true;
        desc[world.space.index(Coord::new(x, y, Layer::Underground))] = true;
    }
    world.set_descendable(desc);
}
```

- [ ] **Step 4: Seed the `dig` trait off**

In `seed_on_fed_land`, the genome array (currently 9 long after Task 1 made it 10 — confirm it ends in `0.0, // swim`) must include `dig`. Ensure the array reads:

```rust
        let g = Genome::from_array([
            0.45,                      // size
            0.7,                       // valaar_efficiency
            0.4,                       // speed
            sim.rng.next_unit() * 0.4, // diet (autotroph-leaning)
            0.5,                       // repro_threshold
            0.5,                       // lifespan
            0.5,                       // heat_tolerance
            0.5,                       // drought_tolerance
            0.0,                       // swim (arises by mutation)
            0.0,                       // dig (arises by mutation)
        ])
        .clamped();
```

- [ ] **Step 5: Run the suite + lint + commit**

```bash
cd ~/dev/alife && cargo test && cargo clippy --all-targets
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: place + wire underground valaar reservoirs (crystalline sink)"
```

---

### Task 8: Wire reservoirs into the live sim (`build_sim`)

**Files:**
- Modify: `src/bin/mapview.rs` (`build_sim`)

**Interfaces:**
- Consumes: `mapsim::{place_underground_reservoirs, add_underground_reservoirs}` (Task 7).
- Produces: no new API — `build_sim` now installs reservoirs.

- [ ] **Step 1: Extend the imports**

In `src/bin/mapview.rs`, add the two functions to the `mapsim` import:

```rust
use alife::mapsim::{
    add_underground_reservoirs, compute_stats, downscale, label_continents, marker_color,
    place_underground_reservoirs, seed_on_fed_land, world_from_materials,
};
```

- [ ] **Step 2: Install reservoirs in `build_sim`**

In `build_sim`, just after `let mut world = world_from_materials(sw, sh, mats);` and the two `world.params` lines, add:

```rust
    let reservoirs = place_underground_reservoirs(sw, sh, continents);
    add_underground_reservoirs(&mut world, &reservoirs);
```

- [ ] **Step 3: Build + lint**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: install underground reservoirs in the live map sim"
```

---

### Task 9: Underground-view flip + layer-aware overlay/organisms (viewer)

**Files:**
- Modify: `src/bin/mapview.rs`

**Interfaces:**
- Consumes: `draw_valaar(.., layer)` and `draw_organisms` (plan 5d + this task); `TileSim`.
- Produces: a `view_layer: Layer` field on `TileSim`, a "Flip layer" button, an underground cave backdrop, and layer-filtered overlays.

- [ ] **Step 1: Add the `view_layer` field**

In `TileSim`, add (near `view`/render state):

```rust
    view_layer: Layer,
```

In `build_tile_scene`'s `TileSim { ... }` literal, add:

```rust
        view_layer: Layer::Surface,
```

- [ ] **Step 2: Add a constant for the cave backdrop**

Near the other `const`s at the top of `bin/mapview.rs`:

```rust
const CAVE_BACKDROP: u32 = 0x000A_0A12; // dark rock for the Underground view
```

- [ ] **Step 3: Filter organisms by the viewed layer**

In `draw_organisms`, change the loop so only organisms on the viewed layer draw. Pass the layer in: update the signature to `fn draw_organisms(painter, rect, cam, t, layer: Layer)` and add, right after `for o in t.sim.pop.organisms() {`:

```rust
        if o.pos.layer != layer {
            continue;
        }
```

- [ ] **Step 4: Render the cave backdrop for the underground view**

In `render_terrain`, change the `Scene::Tiles` arm to branch on the viewed layer:

```rust
        Scene::Tiles(t) => {
            if t.view_layer == Layer::Underground {
                buf.iter_mut().for_each(|p| *p = CAVE_BACKDROP);
            } else {
                // Half-tile offset: autotiled tiles are a dual grid; shift +½ cell.
                let tile_cam = Camera { cx: cam.cx + 0.5, cy: cam.cy + 0.5, zoom: cam.zoom };
                render_tiles_to_buffer(&t.map, &t.atlas, &tile_cam, vw, vh, buf);
            }
        }
```

- [ ] **Step 5: Make the overlays layer-aware in the central panel**

Replace the overlay block from plan 5d with one that uses `t.view_layer` and skips surface-only bridges underground:

```rust
            if let Scene::Tiles(t) = &self.scene {
                let painter = ui.painter_at(rect); // clip overlays to the map rect
                if t.view_layer == Layer::Surface {
                    draw_bridges(&painter, rect, &self.cam, t);
                }
                if t.show_valaar {
                    draw_valaar(&painter, rect, &self.cam, t, t.view_layer);
                }
                draw_organisms(&painter, rect, &self.cam, t, t.view_layer);
            }
```

- [ ] **Step 6: Add the flip button in the side panel**

In the `Scene::Tiles(t)` panel arm, next to the Pause/Reseed `ui.horizontal`, add a button that toggles the layer:

```rust
                    ui.horizontal(|ui| {
                        let label = match t.view_layer {
                            Layer::Surface => "Flip to underground",
                            Layer::Underground => "Flip to surface",
                        };
                        if ui.button(label).clicked() {
                            t.view_layer = match t.view_layer {
                                Layer::Surface => Layer::Underground,
                                Layer::Underground => Layer::Surface,
                            };
                        }
                    });
```

- [ ] **Step 7: Build + lint + eyeball**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Then `cargo run --release --bin mapview`: tick **Valaar**, click **Flip to underground** — the map goes dark and two glowing reservoir pools appear at the far south; once `dig` mutates in (give it time, or raise `mutation_rate`), circles appear sheltering in the pools. Flip back to see the surface.

- [ ] **Step 8: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: underground-view flip — cave backdrop + reservoirs + diggers below"
```

---

### Task 10: `dig_drain` slider + lore + README

**Files:**
- Modify: `src/bin/mapview.rs` (`parameters_ui`)
- Modify: `~/dev/ideas/world-design.md` (lore)
- Modify: `README.md` (status rows)

- [ ] **Step 1: Add the `dig_drain` slider**

In `parameters_ui`, in the "Ecology (live)" block, after the `valaar_drain` slider add:

```rust
        slider_f32(ui, "dig_drain", &mut e.dig_drain, 0.0..=1.0);
```

- [ ] **Step 2: Confirm the lore is in `world-design.md`**

The crystalline-sink lore was added to `~/dev/ideas/world-design.md` §F on 2026-06-21 (the
"**The southern sinks — where the underground reservoirs come from**" paragraph, right after the
two-layer 2.5D paragraph). Verify it is present and matches this plan (SW tip of the major
continent + southmost island; Vraze crystal sinks + thaws; `digging` descends to shelter/forage).
If it is missing, re-add it. **Do not duplicate it.**

- [ ] **Step 3: Update the README status table**

In `README.md`, under the `## Status` table, add rows (and adjust the `5+` row to note what's now shipped):

```markdown
| 5d | Valaar phase overlay — the field made visible (amount→opacity, phase→hue, crystal tint) | ✅ done |
| 6 | Subterranean refuge — `dig` gene + underground valaar reservoirs (the crystalline sink) + underground-view flip | ✅ done |
```

- [ ] **Step 4: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: dig_drain slider + southern-sink lore + README status"
```

---

## Self-Review

- **Spec coverage:** `dig` gene (Task 1, D1); `can_dig` (Task 2, D1); `dig_drain` (Task 3, D7); descendable mask (Task 4, D2); descent/ascent gradient + cost (Task 5, D2/D3); underground refuge (Task 6, D4); reservoir placement at SW-tip + southmost island as passable sources/access points (Task 7, D5/D6); live wiring (Task 8); underground-view flip + diggers + reused overlay (Task 9); slider + lore + README (Task 10). Covered.
- **Determinism:** descent lives inside the existing once-per-organism `rng.next_unit()` gate (stream unchanged); placement uses no RNG. Holds. (Genome arity change intentionally alters runs vs. pre-plan-6.)
- **Placeholder scan:** every code step shows complete code; the compiler-driven array fix-up (Task 1 Step 4) names exact files and the exact edit (append a 10th element). No TBDs.
- **Type consistency:** `move_organisms(.., descendable)` 8-arg signature matches the `Sim::step` call (Task 5 Step 5) and all test calls (Task 5 Steps 1–2); `place_underground_reservoirs(sw, sh, labels) -> Vec<(u32,u32)>` and `add_underground_reservoirs(world, cols)` match their call sites (Task 8); `draw_organisms(.., layer)` / `draw_valaar(.., layer)` signatures match the central-panel calls (Task 9 Step 5); `can_dig`/`DIG_THRESHOLD`/`dig_drain`/`descendable`/`view_layer` names consistent throughout.
- **Dependency on plan 5d:** Tasks 9–10 call `draw_valaar` and reuse the overlay helpers from plan 5d; do plan 5d first.
