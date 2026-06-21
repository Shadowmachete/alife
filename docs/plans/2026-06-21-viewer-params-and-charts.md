# Plan 5c — Viewer Parameter Sliders + Charts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the `mapview` viewer a collapsible **Parameters** panel (sliders for every dial, with live ecology tuning + a **Reload** button for a fresh A/B run) and a **Charts** bottom panel that plots population and mean body-size trends over time — overall plus per continent, each series toggleable.

**Architecture:** Two new lib units, both unit-tested: `mapsim::Stats` gains per-continent **mean size** (`ContinentStat`), and a new `history::History` ring buffer stores capped time-series snapshots and emits `egui_plot`-ready series. The viewer holds a `Tunables` working copy of `EcoParams` + valaar `Params` + `BridgeConfig`; ecology sliders are copied into `sim.eco` live each frame, while structural (valaar / bridge) sliders take effect on **Reload**, which rebuilds the scene via a shared `build_sim` helper (new world → recomputed bridges → fresh `Sim` → warm → reseed). Charts read the `History` buffer.

**Tech Stack:** Rust (edition 2021), `eframe`/`egui` 0.29. **One new dependency:** `egui_plot = "0.29"` (matches the egui version; works native + WASM).

**Project root:** `~/dev/alife/`. All paths relative to it.

**Design source:** brainstorm of 2026-06-21 (this session). User decisions: *everything tunable*; Reload does *both* (live-apply ecology + full rebuild); charts via *egui_plot*; charts show total/mean + per-continent, toggleable.

**Builds on (shipped — exact interfaces consumed):**

- `mapsim::{compute_stats, Stats, label_continents, downscale, world_from_materials, seed_on_fed_land, marker_color}` — `compute_stats(pop, sw, labels, n_continents) -> Stats`; `Stats { total, autotrophs, predators, mean_size, continents }`; `world_from_materials(sw, sh, &mats) -> World<Grid2p5D>`.
- `params::EcoParams` — Copy struct; fields `base_energy, size_energy, basal_cost, size_cost, uptake_rate, move_cost, detritus_fraction, repro_cost_fraction, mutation_rate, min_lifespan: u32, max_lifespan: u32, predation_efficiency, heat_stress, drought_stress, initial_energy, valaar_drain, rasgun_mutation_mult`.
- `world::{World, Params}` — `Params { source_rate, diffuse_rate, layer_exchange, decay }`, Default `1.0/0.2/0.1/0.01`; `World.params` public.
- `bridges::{BridgeConfig, find_bridge_sites, Bridges}` — `BridgeConfig { max_gap, min_width, max_width: u32, site_fraction, open_fraction: f32, min_duration, max_duration: u32 }`.
- `sim::Sim<S>` — `Sim::new(world, eco, seed)`; pub `world`/`pop`/`eco`/`calendar`; `world.space: Grid2p5D` with `Space::{width, height}`.
- `season::CRAWS_PER_YEAR: u32`.
- `rng::Rng::new(seed)`.
- `bin/mapview.rs` — `struct TileSim { map, atlas, sim: Sim<Grid2p5D>, mats: Vec<CellType>, continents: Vec<Option<u32>>, n_continents: u32, running: bool }`; `TileSim::reseed`; `build_tile_scene(xml, atlas_bytes) -> Scene`; consts `SIM_SCALE=3`, `SEED_COUNT=1500`, `WARM_STEPS=150`; the right `SidePanel` shows stats + Pause/Reseed; `update()` calls `t.sim.step()` then renders terrain + `draw_bridges`/`draw_organisms`. Sim built with seeds `0xA11FE` (sim) and `0xB12D6E` (bridges).

## Global Constraints

- **Language:** Rust, edition 2021. One new dependency only: `egui_plot = "0.29"`.
- **Formatting / lint gate (repo convention):** repo is **not** rustfmt-clean — **do NOT run `cargo fmt`**. Hand-format to match surrounding code. Gate every task on `cargo test` and `cargo clippy --all-targets` (clippy must be clean). Viewer-wiring tasks additionally gate on `cargo build --bin mapview` and a manual run.
- **Layering:** logic (`Stats` extension, `History`) lives in lib modules with unit tests; the sliders/charts are `egui` wiring in `bin/mapview.rs`, verified by build + clippy + a manual run (no unit tests for UI).
- **Determinism:** Reload rebuilds with fixed seeds (`SIM_SEED`, `BRIDGE_SEED`), so `(tunables, seeds)` is reproducible. Live ecology edits change `sim.eco` only (read each tick); no new RNG draws.
- **WASM:** `egui_plot` and all viewer code must stay `wasm32`-clean (no `std::fs`, no threads in the new code). `build_tile_scene` is shared by the native and wasm loaders — keep it target-agnostic.
- **Version control:** work on `main` (per user). Commit per task, plain message, **no `Co-Authored-By` trailer**. `git add` only the files each task names. The working tree carries unrelated user tuning edits in `params.rs`/`bridges.rs` — leave them unstaged.

---

## Design assumptions (flagged for veto)

- **A1 — Live vs Reload split.** Ecology (`EcoParams`) sliders apply **live** (copied into `sim.eco` each frame). Valaar (`diffuse_rate`, `decay`) and Bridge sliders are **structural** → applied on **Reload** only. Reload = full fresh rebuild (new world + recomputed bridges + new sim + warm + reseed) on the same map.
- **A2 — Charts.** Two stacked `egui_plot` charts in a collapsible **bottom** panel: **Population** (y = count) and **Mean size** (y = body size), x = absolute craw. Series: an aggregate (**total** / **overall mean**) + one per continent, each toggled by a checkbox. Continents share one visibility vector across both charts.
- **A3 — Sampling.** Snapshot every `SAMPLE_EVERY = 5` craws while running, into a ring buffer capped at `HISTORY_CAP = 4000` (drops oldest). Reload clears history.
- **A4 — Scope.** No autotroph/predator series (total + per-continent only). No preset save/load to disk. Crystal/phase visualisation (plan 5b) is not part of this plan.

---

## File Structure

- `src/mapsim.rs` — **modified**: `ContinentStat`; `Stats.continents: Vec<ContinentStat>`; `compute_stats` per-continent mean size; update its test.
- `src/history.rs` — **new**: `History`, `Snapshot`, `ContinentPoint` + tests.
- `src/lib.rs` — **modified**: register `pub mod history;`.
- `Cargo.toml` — **modified**: add `egui_plot = "0.29"`.
- `src/bin/mapview.rs` — **modified**: `Tunables`; consts; `build_sim` helper; `TileSim` new fields; Parameters panel + live ecology; `rebuild` + Reload; History sampling + Charts panel.
- `README.md` — **modified**: note the viewer controls.

---

### Task 1: per-continent mean size in `Stats`

**Files:**
- Modify: `src/mapsim.rs` (`ContinentStat`, `Stats`, `compute_stats`, its test)
- Modify: `src/bin/mapview.rs` (the one stats-panel consumer)

**Interfaces:**
- Produces: `ContinentStat { label: u32, count: usize, mean_size: f32 }` (Clone, Debug, PartialEq); `Stats.continents: Vec<ContinentStat>`.

- [ ] **Step 1: Update the failing test**

In `src/mapsim.rs`, replace the final assertion of `compute_stats_tallies_diet_size_and_continents`:

```rust
        assert_eq!(s.continents, vec![(0, 2), (1, 1)]);
```

with:

```rust
        assert_eq!(s.continents.len(), 2);
        assert_eq!((s.continents[0].label, s.continents[0].count), (0, 2));
        assert!((s.continents[0].mean_size - 0.3).abs() < 1e-6); // (0.2 + 0.4) / 2
        assert_eq!((s.continents[1].label, s.continents[1].count), (1, 1));
        assert!((s.continents[1].mean_size - 0.6).abs() < 1e-6);
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib mapsim`
Expected: FAIL — no field `label` on tuple / type mismatch (`ContinentStat` undefined).

- [ ] **Step 3: Add `ContinentStat` and extend `Stats`**

In `src/mapsim.rs`, add above `struct Stats`:

```rust
/// Per-continent tally: population and mean body size.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinentStat {
    pub label: u32,
    pub count: usize,
    pub mean_size: f32,
}
```

Change the `continents` field of `Stats`:

```rust
    /// Per-continent tally, sorted by population desc then label asc.
    pub continents: Vec<ContinentStat>,
```

- [ ] **Step 4: Compute per-continent mean size**

In `compute_stats`, replace the per-continent accumulation and assembly. Replace:

```rust
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
```

with:

```rust
    let mut per_count = vec![0usize; n_continents as usize];
    let mut per_size = vec![0.0f32; n_continents as usize];
    for o in orgs {
        if o.genome.diet <= 0.5 {
            autotrophs += 1;
        }
        size_sum += o.genome.size;
        let idx = (o.pos.y * sw + o.pos.x) as usize;
        if let Some(Some(label)) = labels.get(idx) {
            per_count[*label as usize] += 1;
            per_size[*label as usize] += o.genome.size;
        }
    }
    let mean_size = if total > 0 { size_sum / total as f32 } else { 0.0 };
    let mut continents: Vec<ContinentStat> = (0..n_continents as usize)
        .map(|l| ContinentStat {
            label: l as u32,
            count: per_count[l],
            mean_size: if per_count[l] > 0 { per_size[l] / per_count[l] as f32 } else { 0.0 },
        })
        .collect();
    continents.sort_by(|a, b| b.count.cmp(&a.count).then(a.label.cmp(&b.label)));
    Stats { total, autotrophs, predators: total - autotrophs, mean_size, continents }
```

- [ ] **Step 5: Update the viewer's stats-panel consumer**

In `src/bin/mapview.rs`, replace the per-continent loop in the right side panel:

```rust
                    for (label, count) in s.continents.iter().filter(|(_, c)| *c > 0).take(8) {
                        ui.label(format!("  c{label}: {count}"));
                    }
```

with:

```rust
                    for cs in s.continents.iter().filter(|cs| cs.count > 0).take(8) {
                        ui.label(format!("  c{}: {} (size {:.2})", cs.label, cs.count, cs.mean_size));
                    }
```

- [ ] **Step 6: Run, lint, build**

Run: `cd ~/dev/alife && cargo test --lib mapsim && cargo clippy --all-targets && cargo build --bin mapview`
Expected: PASS, clippy clean, mapview builds.

- [ ] **Step 7: Commit**

```bash
git add src/mapsim.rs src/bin/mapview.rs
git commit -m "feat: per-continent mean size in Stats"
```

---

### Task 2: `History` time-series buffer

**Files:**
- Create: `src/history.rs`
- Modify: `src/lib.rs` (register module)

**Interfaces:**
- Produces: `ContinentPoint { label: u32, count: usize, mean_size: f32 }`; `Snapshot { x: f64, total: usize, mean_size: f32, continents: Vec<ContinentPoint> }`; `History::{new(cap), clear, len, is_empty, push(Snapshot), total_series, mean_size_series, continent_count_series(label), continent_size_series(label)}` returning `Vec<[f64; 2]>`.

- [ ] **Step 1: Register the module**

In `src/lib.rs`, add after `pub mod bridges;`:

```rust
pub mod history;
```

- [ ] **Step 2: Write the module with failing tests**

Create `src/history.rs`:

```rust
//! A capped time-series of population snapshots for the viewer's charts. Pure
//! data: the viewer pushes a `Snapshot` each sample and reads back `egui_plot`-
//! ready `Vec<[f64; 2]>` series.

/// One continent's data point within a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinentPoint {
    pub label: u32,
    pub count: usize,
    pub mean_size: f32,
}

/// One sampled moment in time.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// X coordinate for the plot (absolute craw).
    pub x: f64,
    pub total: usize,
    pub mean_size: f32,
    pub continents: Vec<ContinentPoint>,
}

/// A capped buffer of snapshots; oldest points drop once `cap` is exceeded.
pub struct History {
    points: Vec<Snapshot>,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> Self {
        History { points: Vec::new(), cap }
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Append a snapshot, dropping the oldest if over capacity.
    pub fn push(&mut self, s: Snapshot) {
        self.points.push(s);
        if self.points.len() > self.cap {
            let overflow = self.points.len() - self.cap;
            self.points.drain(0..overflow);
        }
    }

    /// `(x, total population)` over time.
    pub fn total_series(&self) -> Vec<[f64; 2]> {
        self.points.iter().map(|s| [s.x, s.total as f64]).collect()
    }

    /// `(x, overall mean body size)` over time.
    pub fn mean_size_series(&self) -> Vec<[f64; 2]> {
        self.points.iter().map(|s| [s.x, s.mean_size as f64]).collect()
    }

    /// `(x, population)` over time for one continent label (points where the
    /// continent was present).
    pub fn continent_count_series(&self, label: u32) -> Vec<[f64; 2]> {
        self.points
            .iter()
            .filter_map(|s| s.continents.iter().find(|c| c.label == label).map(|c| [s.x, c.count as f64]))
            .collect()
    }

    /// `(x, mean body size)` over time for one continent label.
    pub fn continent_size_series(&self, label: u32) -> Vec<[f64; 2]> {
        self.points
            .iter()
            .filter_map(|s| s.continents.iter().find(|c| c.label == label).map(|c| [s.x, c.mean_size as f64]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(x: f64, total: usize) -> Snapshot {
        Snapshot {
            x,
            total,
            mean_size: 0.5,
            continents: vec![ContinentPoint { label: 0, count: total, mean_size: 0.5 }],
        }
    }

    #[test]
    fn push_caps_and_drops_oldest() {
        let mut h = History::new(3);
        for i in 0..5 {
            h.push(snap(i as f64, i));
        }
        assert_eq!(h.len(), 3);
        // oldest two (x=0,1) dropped; series starts at x=2.
        let series = h.total_series();
        assert_eq!(series.first().unwrap()[0], 2.0);
        assert_eq!(series.last().unwrap()[0], 4.0);
    }

    #[test]
    fn series_extract_x_and_value() {
        let mut h = History::new(10);
        h.push(snap(0.0, 7));
        h.push(snap(5.0, 9));
        assert_eq!(h.total_series(), vec![[0.0, 7.0], [5.0, 9.0]]);
        assert_eq!(h.continent_count_series(0), vec![[0.0, 7.0], [5.0, 9.0]]);
        assert!(h.continent_count_series(99).is_empty(), "absent continent => empty");
    }

    #[test]
    fn clear_empties() {
        let mut h = History::new(10);
        h.push(snap(0.0, 1));
        h.clear();
        assert!(h.is_empty());
    }
}
```

- [ ] **Step 3: Run to verify it passes**

Run: `cd ~/dev/alife && cargo test --lib history`
Expected: PASS (3 tests). (They are the failing-first gate: the module didn't compile before it existed.)

- [ ] **Step 4: Lint**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/history.rs
git commit -m "feat: History time-series buffer for viewer charts"
```

---

### Task 3: `egui_plot` dep + `Tunables` + Parameters panel (live ecology)

**Files:**
- Modify: `Cargo.toml` (add `egui_plot`)
- Modify: `src/bin/mapview.rs` (imports, consts, `Tunables`, `TileSim.tunables`, a slider helper, Parameters panel, live ecology apply)

> Viewer wiring — no unit test. Gate: full suite still green + clippy clean + `cargo build --bin mapview` + a manual run.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]` after `eframe = "0.29"`:

```toml
eframe = "0.29"
egui_plot = "0.29"
```

- [ ] **Step 2: Add imports + seed/cadence consts**

In `src/bin/mapview.rs`, add to the `use alife::...` imports:

```rust
use alife::world::Params;
```

Below the existing consts (`SIM_SCALE`, `SEED_COUNT`, `WARM_STEPS`), add:

```rust
const SIM_SEED: u64 = 0xA11FE;
const BRIDGE_SEED: u64 = 0xB12D6E;
const HISTORY_CAP: usize = 4000;
const SAMPLE_EVERY: u32 = 5;
```

- [ ] **Step 3: Add the `Tunables` struct + slider helpers**

In `src/bin/mapview.rs`, add near the top-level types (e.g. above `struct TileSim`):

```rust
/// The editable working copy behind the Parameters panel.
#[derive(Clone)]
struct Tunables {
    eco: EcoParams,
    diffuse_rate: f32,
    decay: f32,
    bridges: BridgeConfig,
}

impl Default for Tunables {
    fn default() -> Self {
        let p = Params::default();
        Tunables {
            eco: EcoParams::default(),
            diffuse_rate: p.diffuse_rate,
            decay: p.decay,
            bridges: BridgeConfig::default(),
        }
    }
}

fn slider_f32(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.add(egui::Slider::new(v, range).text(label));
}

fn slider_u32(ui: &mut egui::Ui, label: &str, v: &mut u32, range: std::ops::RangeInclusive<u32>) {
    ui.add(egui::Slider::new(v, range).text(label));
}
```

- [ ] **Step 4: Add `tunables` to `TileSim` + initialise it**

In `src/bin/mapview.rs`, add the field to `struct TileSim` after `running: bool,`:

```rust
    running: bool,
    tunables: Tunables,
```

In `build_tile_scene`, set it in the `TileSim { .. }` literal (after `running: true,`):

```rust
        running: true,
        tunables: Tunables::default(),
```

- [ ] **Step 5: Add the Parameters panel + live ecology apply**

In `src/bin/mapview.rs`, add this free function (e.g. after `draw_organisms`):

```rust
/// The collapsible Parameters panel: sliders for every dial. Ecology applies
/// live; valaar/bridge sliders are staged and take effect on Reload (handled by
/// the caller). Returns `true` if the Reload button was clicked.
fn parameters_ui(ui: &mut egui::Ui, tun: &mut Tunables) -> bool {
    let mut reload = false;
    egui::CollapsingHeader::new("Parameters").default_open(false).show(ui, |ui| {
        ui.label(egui::RichText::new("Ecology (live)").strong());
        let e = &mut tun.eco;
        slider_f32(ui, "uptake_rate", &mut e.uptake_rate, 0.0..=1.0);
        slider_f32(ui, "move_cost", &mut e.move_cost, 0.0..=0.2);
        slider_f32(ui, "basal_cost", &mut e.basal_cost, 0.0..=0.2);
        slider_f32(ui, "repro_cost_fraction", &mut e.repro_cost_fraction, 0.0..=1.0);
        slider_f32(ui, "mutation_rate", &mut e.mutation_rate, 0.0..=0.5);
        slider_f32(ui, "rasgun_mutation_mult", &mut e.rasgun_mutation_mult, 1.0..=5.0);
        slider_f32(ui, "predation_efficiency", &mut e.predation_efficiency, 0.0..=1.0);
        slider_f32(ui, "heat_stress", &mut e.heat_stress, 0.0..=1.0);
        slider_f32(ui, "drought_stress", &mut e.drought_stress, 0.0..=1.0);
        slider_f32(ui, "detritus_fraction", &mut e.detritus_fraction, 0.0..=1.0);
        slider_f32(ui, "valaar_drain", &mut e.valaar_drain, 0.0..=1.0);
        slider_f32(ui, "base_energy", &mut e.base_energy, 0.5..=16.0);
        slider_f32(ui, "size_energy", &mut e.size_energy, 0.0..=32.0);
        slider_f32(ui, "size_cost", &mut e.size_cost, 0.0..=0.2);
        slider_f32(ui, "initial_energy", &mut e.initial_energy, 0.5..=16.0);
        slider_u32(ui, "min_lifespan", &mut e.min_lifespan, 1..=1000);
        slider_u32(ui, "max_lifespan", &mut e.max_lifespan, 1..=4000);

        ui.separator();
        ui.label(egui::RichText::new("Valaar (reload)").strong());
        slider_f32(ui, "diffuse_rate", &mut tun.diffuse_rate, 0.0..=0.24);
        slider_f32(ui, "decay", &mut tun.decay, 0.0..=0.2);

        ui.separator();
        ui.label(egui::RichText::new("Bridges (reload)").strong());
        let b = &mut tun.bridges;
        slider_u32(ui, "max_gap", &mut b.max_gap, 1..=30);
        slider_u32(ui, "min_width", &mut b.min_width, 1..=10);
        slider_u32(ui, "max_width", &mut b.max_width, 1..=12);
        slider_f32(ui, "site_fraction", &mut b.site_fraction, 0.0..=1.0);
        slider_f32(ui, "open_fraction", &mut b.open_fraction, 0.0..=1.0);
        slider_u32(ui, "min_duration", &mut b.min_duration, 1..=117);
        slider_u32(ui, "max_duration", &mut b.max_duration, 1..=117);

        ui.separator();
        if ui.button("Reload (fresh run)").clicked() {
            reload = true;
        }
    });
    reload
}
```

In `update()`, inside the `Scene::Tiles(t) =>` arm of the right side panel, after the Pause/Reseed `ui.horizontal(...)` block, add:

```rust
                    ui.separator();
                    let _reload = parameters_ui(ui, &mut t.tunables);
                    t.sim.eco = t.tunables.eco; // ecology applies live
```

(The `_reload` result is wired to the rebuild in Task 4 — for now it compiles unused with the leading underscore.)

- [ ] **Step 6: Build, lint, run**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo test && cargo clippy --all-targets`
Expected: builds, tests pass, clippy clean.

Manual: `cargo run --release --bin mapview` — expand **Parameters**; dragging an ecology slider (e.g. `predation_efficiency`) visibly shifts the population within seconds. Valaar/Bridge sliders move but don't yet act (Reload lands next task).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/bin/mapview.rs
git commit -m "feat: viewer Parameters panel with live ecology sliders"
```

---

### Task 4: Reload — shared `build_sim` + rebuild

**Files:**
- Modify: `src/bin/mapview.rs` (`build_sim` helper, refactor `build_tile_scene`, `TileSim::rebuild`, wire the Reload button)

> Viewer wiring — no unit test. Gate: build + clippy + manual run.

- [ ] **Step 1: Extract a shared `build_sim` helper**

In `src/bin/mapview.rs`, add:

```rust
/// Build a warmed `Sim` (world + bridges) for a tile scene from the downscaled
/// materials and the current tunables. Shared by first load and Reload.
fn build_sim(
    mats: &[CellType],
    sw: u32,
    sh: u32,
    continents: &[Option<u32>],
    tun: &Tunables,
) -> Sim<Grid2p5D> {
    let mut world = world_from_materials(sw, sh, mats);
    world.params.diffuse_rate = tun.diffuse_rate;
    world.params.decay = tun.decay;
    let mut sim = Sim::new(world, tun.eco, SIM_SEED);
    let sites = find_bridge_sites(mats, sw, sh, continents, &mut Rng::new(BRIDGE_SEED), &tun.bridges);
    sim.set_bridges(Bridges::new(sites, tun.bridges, BRIDGE_SEED));
    for _ in 0..WARM_STEPS {
        sim.world.step();
    }
    sim
}
```

- [ ] **Step 2: Use it in `build_tile_scene`**

In `build_tile_scene`, replace the inline world/sim/bridge construction

```rust
    let world = world_from_materials(sw, sh, &sim_mats);
    let mut sim = Sim::new(world, EcoParams::default(), 0xA11FE);
    let bcfg = BridgeConfig::default();
    let sites = find_bridge_sites(&sim_mats, sw, sh, &continents, &mut Rng::new(0xB12D6E), &bcfg);
    sim.set_bridges(Bridges::new(sites, bcfg, 0xB12D6E));
    for _ in 0..WARM_STEPS {
        sim.world.step();
    }
```

with:

```rust
    let tunables = Tunables::default();
    let sim = build_sim(&sim_mats, sw, sh, &continents, &tunables);
```

and set the field from this local in the `TileSim { .. }` literal — change `tunables: Tunables::default(),` (from Task 3) to:

```rust
        running: true,
        tunables,
```

- [ ] **Step 3: Add `TileSim::rebuild`**

In `src/bin/mapview.rs`, add to `impl TileSim` (next to `reseed`):

```rust
    /// Fresh A/B run: rebuild the world + bridges + sim from the current
    /// tunables on the same map, re-warm valaar, and re-seed life.
    fn rebuild(&mut self) {
        let sw = self.sim.world.space.width();
        let sh = self.sim.world.space.height();
        self.sim = build_sim(&self.mats, sw, sh, &self.continents, &self.tunables);
        self.reseed();
    }
```

- [ ] **Step 4: Wire the Reload button**

In `update()`, replace the Task-3 lines

```rust
                    let _reload = parameters_ui(ui, &mut t.tunables);
                    t.sim.eco = t.tunables.eco; // ecology applies live
```

with:

```rust
                    let reload = parameters_ui(ui, &mut t.tunables);
                    t.sim.eco = t.tunables.eco; // ecology applies live
                    if reload {
                        t.rebuild();
                    }
```

- [ ] **Step 5: Build, lint, run**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo test && cargo clippy --all-targets`
Expected: builds, tests pass, clippy clean.

Manual: `cargo run --release --bin mapview` — set `site_fraction`/`open_fraction` to `1.0` and `max_gap` higher, click **Reload**: the run restarts and bridges are more frequent/visible during Vraze.

- [ ] **Step 6: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: Reload rebuilds the sim from tunables (valaar + bridges)"
```

---

### Task 5: History sampling + Charts panel

**Files:**
- Modify: `src/bin/mapview.rs` (imports, `TileSim` chart fields, sampling, charts bottom panel, clear history on rebuild)

> Viewer wiring — no unit test. Gate: build + clippy + manual run.

- [ ] **Step 1: Add imports**

In `src/bin/mapview.rs`:

```rust
use alife::history::{ContinentPoint, History, Snapshot};
use alife::season::CRAWS_PER_YEAR;
use egui_plot::{Legend, Line, Plot, PlotPoints};
```

- [ ] **Step 2: Add chart state to `TileSim`**

Add fields to `struct TileSim` after `tunables: Tunables,`:

```rust
    tunables: Tunables,
    history: History,
    show_charts: bool,
    show_total: bool,
    continent_visible: Vec<bool>,
```

In `build_tile_scene`'s `TileSim { .. }` literal, after `tunables,`:

```rust
        tunables,
        history: History::new(HISTORY_CAP),
        show_charts: false,
        show_total: true,
        continent_visible: vec![true; n_continents as usize],
```

- [ ] **Step 3: Clear history on rebuild**

In `TileSim::rebuild` (Task 4), add as the last line:

```rust
        self.reseed();
        self.history.clear();
    }
```

- [ ] **Step 4: Sample into history each running tick**

In `update()`, right after the `t.sim.step();` call inside `if t.running { .. }`:

```rust
        if let Scene::Tiles(t) = &mut self.scene {
            if t.running {
                t.sim.step();
                if t.sim.calendar.craw() % SAMPLE_EVERY == 0 {
                    let sw = t.sim.world.space.width();
                    let s = compute_stats(&t.sim.pop, sw, &t.continents, t.n_continents);
                    let x = (t.sim.calendar.year() * CRAWS_PER_YEAR + t.sim.calendar.craw()) as f64;
                    let continents = s
                        .continents
                        .iter()
                        .map(|cs| ContinentPoint { label: cs.label, count: cs.count, mean_size: cs.mean_size })
                        .collect();
                    t.history.push(Snapshot { x, total: s.total, mean_size: s.mean_size, continents });
                }
            }
        }
```

(This replaces the existing `if let Scene::Tiles(t) = &mut self.scene { if t.running { t.sim.step(); } }` block at the top of `update`.)

- [ ] **Step 5: Add a Charts toggle to the side panel**

In the `Scene::Tiles(t) =>` side-panel arm, after the Parameters/reload lines, add:

```rust
                    ui.separator();
                    ui.checkbox(&mut t.show_charts, "Charts");
```

- [ ] **Step 6: Add the Charts bottom panel + series toggles**

In `update()`, **before** the `egui::CentralPanel::default()...` block, add:

```rust
        if let Scene::Tiles(t) = &mut self.scene {
            if t.show_charts {
                egui::TopBottomPanel::bottom("charts")
                    .resizable(true)
                    .default_height(240.0)
                    .show(ctx, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut t.show_total, "total / mean");
                            for (label, vis) in t.continent_visible.iter_mut().enumerate() {
                                ui.checkbox(vis, format!("c{label}"));
                            }
                        });

                        ui.label("Population");
                        Plot::new("population_plot")
                            .legend(Legend::default())
                            .height(90.0)
                            .show(ui, |pui| {
                                if t.show_total {
                                    pui.line(Line::new(PlotPoints::from(t.history.total_series())).name("total"));
                                }
                                for (label, vis) in t.continent_visible.iter().enumerate() {
                                    if *vis {
                                        let pts = t.history.continent_count_series(label as u32);
                                        if !pts.is_empty() {
                                            pui.line(Line::new(PlotPoints::from(pts)).name(format!("c{label}")));
                                        }
                                    }
                                }
                            });

                        ui.label("Mean body size");
                        Plot::new("size_plot")
                            .legend(Legend::default())
                            .height(90.0)
                            .show(ui, |pui| {
                                if t.show_total {
                                    pui.line(Line::new(PlotPoints::from(t.history.mean_size_series())).name("mean"));
                                }
                                for (label, vis) in t.continent_visible.iter().enumerate() {
                                    if *vis {
                                        let pts = t.history.continent_size_series(label as u32);
                                        if !pts.is_empty() {
                                            pui.line(Line::new(PlotPoints::from(pts)).name(format!("c{label}")));
                                        }
                                    }
                                }
                            });
                    });
            }
        }
```

- [ ] **Step 7: Build, lint, run**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo test && cargo clippy --all-targets`
Expected: builds, tests pass, clippy clean.

Manual: `cargo run --release --bin mapview` — tick **Charts**: a bottom panel shows population and mean-size lines climbing over time; toggling `total / mean` and per-continent checkboxes shows/hides series; **Reload** clears the charts and starts fresh.

- [ ] **Step 8: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: viewer charts — population + size trends over time"
```

---

### Task 6: Document the viewer controls

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a viewer-controls note**

In `README.md`, in the section describing the `mapview` demo (near the WASM demo / web build mention), add:

```markdown
The map viewer has a collapsible **Parameters** panel (every ecology dial tunes
live; valaar/bridge dials apply on **Reload**, which starts a fresh run on the
same map) and a **Charts** panel plotting population and mean body-size trends
over time — overall and per continent, each series toggleable.
```

- [ ] **Step 2: Lint check and commit**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

```bash
git add README.md
git commit -m "docs: note viewer Parameters + Charts panels"
```

---

## Self-Review

**Spec coverage:**
- Sliders for *everything tunable* (EcoParams + valaar Params + BridgeConfig) under a side dropdown → Task 3 (`parameters_ui`). ✓
- Reload does *both* (live ecology + full rebuild) → Task 3 (live `sim.eco`), Task 4 (`rebuild` via `build_sim`). ✓
- Charts: population + size trends, mean + per continent, toggleable → Task 1 (per-continent mean size), Task 2 (`History`), Task 5 (two plots + checkboxes). ✓
- egui_plot → Task 3 (dependency), Task 5 (plots). ✓
- Determinism on reload (fixed seeds) → Global Constraints + Task 4 (`SIM_SEED`/`BRIDGE_SEED`). ✓

**Placeholder scan:** every code step shows full code (sliders, plots, helpers all spelled out); run steps give command + expected result + manual check; no TBD/TODO. ✓

**Type consistency:** `ContinentStat { label, count, mean_size }` (Task 1) read by the side panel and by Task 5's sampling; `History`/`Snapshot`/`ContinentPoint` (Task 2) consumed unchanged in Task 5; `Tunables { eco, diffuse_rate, decay, bridges }` (Task 3) consumed by `build_sim` (Task 4); `build_sim(mats, sw, sh, continents, &Tunables) -> Sim<Grid2p5D>` matches both call sites (Task 4); `slider_f32/u32` signatures match every call; chart series methods return `Vec<[f64;2]>` matching `PlotPoints::from`. ✓

**Deferred (later plans):** crystal/phase visualisation (plan 5b's data) and autotroph/predator chart series → future viewer iterations. The `digging`/subterranean refuge → **plan 6**.
```
