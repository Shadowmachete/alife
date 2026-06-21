# Valaar Phase Overlay (viewer) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the valaar field *visible* in the live map viewer — a translucent per-cell overlay whose **opacity tracks how much valaar a cell holds** and whose **hue tracks the current `ValaarPhase`** (Liquid / Gaseous / Crystalline / Sparse), with the frozen `crystal` store drawn in its own icy tint — so plan 5/5b's whole valaar state-machine becomes observable.

**Architecture:** Two **pure, std-only helpers** in `src/viewer.rs` (`phase_rgb`, `overlay_alpha` + the crystal constant) carry all the colour logic and are unit-tested headless. The egui `mapview` binary gets a `draw_valaar` painter (mirroring the existing `draw_bridges`) gated behind a new "Valaar" checkbox, plus a phase legend in the side panel. The overlay takes a `Layer` argument so plan 6's underground view reuses it unchanged.

**Tech Stack:** Rust (edition 2021), `eframe`/`egui` (already a dependency). No new crates.

## Global Constraints

- **Language:** Rust, edition 2021.
- **No new dependencies.** Only `viewer.rs` (std-only core) and `bin/mapview.rs` (egui) change.
- **`viewer.rs` stays std-only:** it may `use crate::valaar::ValaarPhase` (engine, std-only) but MUST NOT `use eframe`/`egui`/`serde`. All egui code lives in `bin/mapview.rs`.
- **Read-only overlay:** the overlay reads `world.valaar` / `world.crystal`; it never mutates the sim, the population, or the `Rng`. Determinism is untouched.
- **Formatting:** the repo is **not** rustfmt-clean — do **not** run `cargo fmt`. Hand-format to match surrounding code; gate on `cargo clippy --all-targets`.
- **Version control:** commit per task. Plain commit messages, **no `Co-Authored-By` trailer** (global user preference).
- **Visual-rep convention:** this *is* the visual for the plan-5b valaar phases — treat the overlay as part of "done".

---

## Design assumptions (flagged for veto)

- **V1 — phase hues:** Liquid `[235, 90, 60]` (molten orange-red), Gaseous `[150, 210, 230]` (fog cyan), Crystalline `[120, 160, 235]` (cold blue), Sparse `[150, 120, 160]` (dim violet-grey). Distinct from each other and from the crystal tint. Tunable.
- **V2 — crystal tint:** `CRYSTAL_RGB = [180, 220, 255]` (icy white-blue). A cell where `crystal > valaar` is drawn with this tint instead of the phase hue, so the Vraze freeze reads as frost.
- **V3 — opacity:** linear in `amount / max` (the displayed layer's peak valaar that frame), capped at `OVERLAY_MAX_ALPHA = 200`. A cell with zero valaar is fully transparent (skipped).
- **V4 — normalisation is per-frame, per-layer:** `max` is recomputed each frame over the displayed layer so the gradient stays legible as totals breathe with the season. (No persistence; no history.)
- **V5 — overlay is off by default:** a "Valaar" checkbox in the panel turns it on, like "Charts".

---

## File Structure

- `src/viewer.rs` — **modified**: add `phase_rgb`, `overlay_alpha`, `OVERLAY_MAX_ALPHA`, `CRYSTAL_RGB` (+ tests). Pure, std-only.
- `src/bin/mapview.rs` — **modified**: `draw_valaar` painter, `show_valaar` field + checkbox, phase legend, central-panel call.

---

### Task 1: Pure overlay colour helpers (`viewer.rs`)

**Files:**
- Modify: `src/viewer.rs` (add the helpers above the `tests` module; add tests inside it)

**Interfaces:**
- Consumes: `crate::valaar::ValaarPhase` (plan 5b — `enum { Liquid, Gaseous, Crystalline, Sparse }`).
- Produces:
  - `pub const OVERLAY_MAX_ALPHA: u8 = 200;`
  - `pub const CRYSTAL_RGB: [u8; 3] = [180, 220, 255];`
  - `pub fn phase_rgb(phase: ValaarPhase) -> [u8; 3]`
  - `pub fn overlay_alpha(amount: f32, max: f32) -> u8`

- [ ] **Step 1: Write the failing tests**

Add inside the `tests` module in `src/viewer.rs`:

```rust
    use crate::valaar::ValaarPhase;

    #[test]
    fn every_phase_has_a_distinct_hue_and_none_match_crystal() {
        let mut seen = std::collections::HashSet::new();
        for p in [
            ValaarPhase::Liquid,
            ValaarPhase::Gaseous,
            ValaarPhase::Crystalline,
            ValaarPhase::Sparse,
        ] {
            assert!(seen.insert(phase_rgb(p)), "duplicate hue for {p:?}");
        }
        assert!(!seen.contains(&CRYSTAL_RGB), "crystal tint must be distinct");
    }

    #[test]
    fn alpha_is_zero_without_valaar_or_max() {
        assert_eq!(overlay_alpha(0.0, 10.0), 0);
        assert_eq!(overlay_alpha(5.0, 0.0), 0);
        assert_eq!(overlay_alpha(-1.0, 10.0), 0);
    }

    #[test]
    fn alpha_is_full_at_the_peak_and_linear_below() {
        assert_eq!(overlay_alpha(10.0, 10.0), OVERLAY_MAX_ALPHA);
        assert_eq!(overlay_alpha(20.0, 10.0), OVERLAY_MAX_ALPHA, "clamps above the peak");
        let half = overlay_alpha(5.0, 10.0);
        assert!(
            (half as i32 - (OVERLAY_MAX_ALPHA as i32 / 2)).abs() <= 1,
            "half the peak ~ half alpha, got {half}"
        );
    }

    #[test]
    fn alpha_is_monotonic_in_amount() {
        assert!(overlay_alpha(2.0, 10.0) < overlay_alpha(7.0, 10.0));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd ~/dev/alife && cargo test --lib viewer`
Expected: **compile error** — `cannot find function phase_rgb` / `overlay_alpha`.

- [ ] **Step 3: Write the implementation**

Insert above the `tests` module in `src/viewer.rs` (and add `use crate::valaar::ValaarPhase;` to the module's top `use` block):

```rust
/// Maximum overlay opacity (0..=255), reached at a cell holding the layer's peak
/// valaar. [V3]
pub const OVERLAY_MAX_ALPHA: u8 = 200;

/// Tint for the frozen `crystal` store — an icy white-blue, distinct from every
/// phase hue. Drawn where a cell's crystal outweighs its liquid valaar. [V2]
pub const CRYSTAL_RGB: [u8; 3] = [180, 220, 255];

/// The hue the valaar overlay paints for each phase; opacity carries amount. [V1]
pub fn phase_rgb(phase: ValaarPhase) -> [u8; 3] {
    match phase {
        ValaarPhase::Liquid => [235, 90, 60],
        ValaarPhase::Gaseous => [150, 210, 230],
        ValaarPhase::Crystalline => [120, 160, 235],
        ValaarPhase::Sparse => [150, 120, 160],
    }
}

/// Opacity (0..=`OVERLAY_MAX_ALPHA`) for a cell holding `amount` valaar, linearly
/// normalised by `max` (the displayed layer's peak). Non-positive `amount`/`max`
/// is transparent; `amount >= max` is full. [V3]
pub fn overlay_alpha(amount: f32, max: f32) -> u8 {
    if amount <= 0.0 || max <= 0.0 {
        return 0;
    }
    let frac = (amount / max).clamp(0.0, 1.0);
    (frac * OVERLAY_MAX_ALPHA as f32).round() as u8
}
```

- [ ] **Step 4: Run the tests**

Run: `cd ~/dev/alife && cargo test --lib viewer`
Expected: the four new tests pass alongside the existing `viewer` tests.

- [ ] **Step 5: Lint**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: no warnings in `viewer.rs`.

- [ ] **Step 6: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: valaar overlay colour helpers (phase hue + amount alpha)"
```

---

### Task 2: `draw_valaar` overlay + toggle + phase legend (`bin/mapview.rs`)

**Files:**
- Modify: `src/bin/mapview.rs`

**Interfaces:**
- Consumes: `viewer::{phase_rgb, overlay_alpha, CRYSTAL_RGB}` (Task 1); `valaar::ValaarPhase`; `space::{Coord, Layer, Space}`; the existing `TileSim` and `Camera`/`SIM_SCALE`.
- Produces: a new `show_valaar: bool` field on `TileSim`, a `draw_valaar(painter, rect, cam, t, layer)` fn, a "Valaar" checkbox, and a phase legend label.

- [ ] **Step 1: Add the imports**

In `src/bin/mapview.rs`, extend the existing `use` lines:
- add `Coord` to the `space` import: `use alife::space::{Coord, Grid2p5D, Layer, Space};`
- add `use alife::valaar::ValaarPhase;`
- add `phase_rgb, overlay_alpha, CRYSTAL_RGB` to the viewer import:
  `use alife::viewer::{overlay_alpha, phase_rgb, render_to_buffer, Camera, CRYSTAL_RGB};`

- [ ] **Step 2: Add the `show_valaar` field**

In the `TileSim` struct (near `show_charts: bool`), add:

```rust
    show_valaar: bool,
```

In `build_tile_scene`, where the `TileSim { ... }` literal is built (near `show_charts: false,`), add:

```rust
        show_valaar: false,
```

- [ ] **Step 3: Add the `draw_valaar` painter**

Add this function next to `draw_bridges` in `src/bin/mapview.rs`:

```rust
/// Paint a translucent valaar overlay over the map for `layer`: per sim-cell, a
/// rectangle whose opacity tracks the cell's valaar (normalised by the layer's
/// peak) and whose hue is the current phase. Cells where frozen `crystal`
/// outweighs liquid valaar are drawn in the icy `CRYSTAL_RGB` instead.
fn draw_valaar(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim, layer: Layer) {
    let space = &t.sim.world.space;
    let (sw, sh) = (space.width(), space.height());
    let hue = phase_rgb(ValaarPhase::for_season(t.sim.season()));
    // Per-frame, per-layer peak for normalisation [V4].
    let mut max_v = 0.0f32;
    for y in 0..sh {
        for x in 0..sw {
            let i = space.index(Coord::new(x, y, layer));
            max_v = max_v.max(t.sim.world.valaar.get(i)).max(t.sim.world.crystal.get(i));
        }
    }
    let cell_px = cam.zoom * SIM_SCALE as f32;
    for y in 0..sh {
        for x in 0..sw {
            let i = space.index(Coord::new(x, y, layer));
            let v = t.sim.world.valaar.get(i);
            let cr = t.sim.world.crystal.get(i);
            let (rgb, amount) = if cr > v { (CRYSTAL_RGB, cr) } else { (hue, v) };
            let a = overlay_alpha(amount, max_v);
            if a == 0 {
                continue;
            }
            let dx = (x * SIM_SCALE) as f32;
            let dy = (y * SIM_SCALE) as f32;
            let sx = rect.min.x + (dx - cam.cx) * cam.zoom;
            let sy = rect.min.y + (dy - cam.cy) * cam.zoom;
            let cell = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(cell_px, cell_px));
            if rect.intersects(cell) {
                painter.rect_filled(
                    cell,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], a),
                );
            }
        }
    }
}
```

- [ ] **Step 4: Add the checkbox + phase legend in the side panel**

In `eframe::App::update`, inside the `Scene::Tiles(t)` arm of the side panel, find the existing `ui.checkbox(&mut t.show_charts, "Charts");` and add directly after it:

```rust
                    ui.checkbox(&mut t.show_valaar, "Valaar");
                    if t.show_valaar {
                        let phase = ValaarPhase::for_season(t.sim.season());
                        let [r, g, b] = phase_rgb(phase);
                        ui.horizontal(|ui| {
                            let (rectw, _) = (12.0, 12.0);
                            let (resp, painter) =
                                ui.allocate_painter(egui::vec2(rectw, rectw), egui::Sense::hover());
                            painter.rect_filled(resp.rect, 2.0, egui::Color32::from_rgb(r, g, b));
                            ui.label(format!("phase: {phase:?}"));
                        });
                    }
```

- [ ] **Step 5: Call `draw_valaar` in the central panel**

In the `CentralPanel`, find the block that paints overlays:

```rust
            if let Scene::Tiles(t) = &self.scene {
                let painter = ui.painter_at(rect); // clip overlays to the map rect
                draw_bridges(&painter, rect, &self.cam, t);
                draw_organisms(&painter, rect, &self.cam, t);
            }
```

Replace it with (overlay drawn *under* organisms, on the surface layer for now — plan 6 passes the viewed layer):

```rust
            if let Scene::Tiles(t) = &self.scene {
                let painter = ui.painter_at(rect); // clip overlays to the map rect
                draw_bridges(&painter, rect, &self.cam, t);
                if t.show_valaar {
                    draw_valaar(&painter, rect, &self.cam, t, Layer::Surface);
                }
                draw_organisms(&painter, rect, &self.cam, t);
            }
```

- [ ] **Step 6: Build + lint**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: builds clean, no new clippy warnings. (CI verifies the binary by building; there is no window in tests.)

- [ ] **Step 7: Eyeball it (manual, optional but recommended)**

Run: `cd ~/dev/alife && cargo run --release --bin mapview`
Tick **Valaar**: a warm gradient should glow brightest at the Rasconne reservoir/rivers and fade toward the Dusk. Let the calendar reach **Vraze** — the tint shifts to cold blue and frosty `crystal` patches appear; in **Miscre** it pales to fog cyan and spreads further. The legend names the active phase.

- [ ] **Step 8: Commit**

```bash
git -C ~/dev/alife add -A && git -C ~/dev/alife commit -m "feat: viewer valaar overlay — phase-tinted amount heatmap + crystal"
```

---

## Self-Review

- **Spec coverage:** amount→alpha (Task 1 `overlay_alpha`, V3/V4); phase→hue (Task 1 `phase_rgb`, V1); crystal tint (Task 1 `CRYSTAL_RGB`, Task 2 crystal branch, V2); toggle off by default (Task 2, V5); phase legend (Task 2 Step 4); layer-aware for plan 6 reuse (Task 2 `draw_valaar(.., layer)`). Covered.
- **Determinism:** overlay only reads fields; no `Rng`, no sim mutation. Untouched.
- **Std-only core:** colour logic in `viewer.rs` imports only `ValaarPhase`; all egui stays in the bin. Holds.
- **Type consistency:** `phase_rgb`/`overlay_alpha`/`CRYSTAL_RGB`/`OVERLAY_MAX_ALPHA` names identical across Tasks 1 and 2. `draw_valaar(painter, rect, cam, t, layer)` matches the call site.
