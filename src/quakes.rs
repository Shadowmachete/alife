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
    /// Half-width (cells) of a quake's fracture band: pool cells within this
    /// perpendicular distance of the random fracture line erupt. Larger = a wider
    /// strip taps more of the pool per quake.
    pub fracture_half_width: f32,
}

impl Default for QuakeConfig {
    fn default() -> Self {
        QuakeConfig {
            erupt_fraction: 0.6,
            release_fraction: 0.8,
            min_duration: 20,
            max_duration: 40,
            fracture_half_width: 1.0,
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
    /// Cells fractured by the current eruption, per pool (empty when inactive) —
    /// the strip the viewer draws, and the only cells that erupted.
    fracture: Vec<Vec<(u32, u32)>>,
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
            fracture: vec![Vec::new(); n],
            scheduled_year: None,
            rng: Rng::new(seed),
        }
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    /// Surface-plane cells of every currently-fracturing pool's strip (viewer
    /// overlay) — the fracture bands, not the whole pools.
    pub fn active_cells(&self) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        for i in 0..self.pools.len() {
            if self.active[i] {
                out.extend_from_slice(&self.fracture[i]);
            }
        }
        out
    }

    /// Crack pool `i` open along a random-angle band through a random pool cell:
    /// every pool cell within `fracture_half_width` of that line bursts its
    /// underground valaar up to the surface. Returns the fractured cells.
    fn fracture_burst<S: Space>(&mut self, i: usize, space: &S, valaar: &mut Field) -> Vec<(u32, u32)> {
        let cells = &self.pools[i].cells;
        if cells.is_empty() {
            return Vec::new();
        }
        // Random fracture line: angle `theta` through a random anchor cell, so
        // successive quakes tap different strips of the same pool.
        let theta = self.rng.next_unit() * std::f32::consts::PI;
        let ai = ((self.rng.next_unit() * cells.len() as f32) as usize).min(cells.len() - 1);
        let (ax, ay) = cells[ai];
        let (nx, ny) = (-theta.sin(), theta.cos()); // unit normal to the line
        let half = self.cfg.fracture_half_width;
        let frac: Vec<(u32, u32)> = cells
            .iter()
            .copied()
            .filter(|&(x, y)| {
                let d = (x as f32 - ax as f32) * nx + (y as f32 - ay as f32) * ny;
                d.abs() <= half
            })
            .collect();
        let release = self.cfg.release_fraction;
        for &(x, y) in &frac {
            let si = space.index(Coord::new(x, y, Layer::Surface));
            let ui = space.index(Coord::new(x, y, Layer::Underground));
            let amt = valaar.get(ui) * release;
            if amt > 0.0 {
                valaar.add(ui, -amt);
                valaar.add(si, amt);
            }
        }
        frac
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
    /// pool becomes active, cracks a **random-angle fracture band** open and
    /// bursts that strip's underground valaar up to the surface. Returns the
    /// cells that erupted this tick.
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
                let frac = self.fracture_burst(i, space, valaar);
                upd.erupted.extend_from_slice(&frac);
                self.fracture[i] = frac;
            } else if !want_active && self.active[i] {
                self.active[i] = false;
                self.fracture[i].clear();
            }
        }
        upd
    }
}

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
            fracture_half_width: 1.0,
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
    fn a_quake_fractures_only_a_band_not_the_whole_pool() {
        // A 7x7 world; the pool covers every cell. One quake should crack open
        // only a strip, leaving most of the pool's underground valaar intact.
        let space = Grid2p5D::new(7, 7);
        let cells: Vec<(u32, u32)> = (0..7).flat_map(|y| (0..7).map(move |x| (x, y))).collect();
        let mut valaar = Field::zeros(space.len());
        for &(x, y) in &cells {
            valaar.set(space.index(Coord::new(x, y, Layer::Underground)), 1.0);
        }
        let cfg = QuakeConfig {
            erupt_fraction: 1.0,
            release_fraction: 1.0,
            min_duration: 10,
            max_duration: 10,
            fracture_half_width: 1.0,
        };
        let mut q = Quakes::new(vec![ReservoirPool { cells: cells.clone() }], cfg, 7);
        let mut erupted: Vec<(u32, u32)> = Vec::new();
        let mut c = cal_at(3 * CRAWS_PER_ARH);
        for _ in 0..CRAWS_PER_ARH {
            let upd = q.update(&c, &space, &mut valaar);
            if !upd.erupted.is_empty() {
                erupted = upd.erupted.clone();
                // While fracturing, the viewer overlay is exactly the burst strip.
                assert_eq!(q.active_cells().len(), erupted.len(), "active strip = burst");
            }
            c.advance();
        }
        assert!(!erupted.is_empty(), "the quake cracked something open");
        assert!(
            erupted.len() < cells.len(),
            "only a band erupts, not the whole pool: {} of {}",
            erupted.len(),
            cells.len()
        );
        // A cell outside the band keeps its underground valaar.
        let untouched = cells.iter().find(|c| !erupted.contains(c)).unwrap();
        let ui = space.index(Coord::new(untouched.0, untouched.1, Layer::Underground));
        assert_eq!(valaar.get(ui), 1.0, "unfractured cells stay untapped");
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
