//! Bridge from a drawn material map to a runnable [`Sim`]: downscale the display
//! grid to a coarser sim grid, seed the valaar field from the map's `Valaar`
//! cells, and seed a population on warmed land.
//!
//! This is the wiring that connects the authored map to the substrate. (Terrain
//! `conductivity`/`passability` are not yet fed into diffusion/movement — a
//! follow-up; for now organisms are simply seeded on fed land and follow the
//! valaar gradient, which keeps them off the foodless ocean.)

use crate::genome::Genome;
use crate::organism::TraitOrganism;
use crate::population::Population;
use crate::sim::Sim;
use crate::space::{Coord, Grid2p5D, Layer, Space};
use crate::terrain::CellType;
use crate::world::{Params, World};

/// Downscale a `dw×dh` material grid by `scale` (centre sample) to a sim grid.
pub fn downscale(mats: &[CellType], dw: u32, dh: u32, scale: u32) -> (u32, u32, Vec<CellType>) {
    let scale = scale.max(1);
    let (sw, sh) = (dw / scale, dh / scale);
    let mut out = Vec::with_capacity((sw * sh) as usize);
    for sy in 0..sh {
        for sx in 0..sw {
            let dx = (sx * scale + scale / 2).min(dw - 1);
            let dy = (sy * scale + scale / 2).min(dh - 1);
            out.push(mats[(dy * dw + dx) as usize]);
        }
    }
    (sw, sh, out)
}

/// A `World` on the sim grid with every `Valaar` cell registered as a surface
/// source — so valaar flows out of the drawn reservoir and rivers — and a
/// passability mask derived from the materials (surface block from `mats`,
/// underground block all-passable).
pub fn world_from_materials(sw: u32, sh: u32, mats: &[CellType]) -> World<Grid2p5D> {
    let mut world = World::new(Grid2p5D::new(sw, sh), Params::default());
    for y in 0..sh {
        for x in 0..sw {
            if mats[(y * sw + x) as usize] == CellType::Valaar {
                world.add_source(Coord::new(x, y, Layer::Surface));
            }
        }
    }
    let plane = (sw * sh) as usize;
    let mut mask = vec![true; world.space.len()]; // both layers; underground all-passable
    for i in 0..plane {
        mask[i] = mats[i].passable(); // surface block occupies indices 0..plane
    }
    world.set_passability(mask);
    let mut swim = vec![false; world.space.len()]; // underground: nothing swimmable
    for i in 0..plane {
        swim[i] = mats[i] == CellType::Valaar; // only Valaar opens to swimmers
    }
    world.set_swimmable(swim);
    world
}

/// Seed up to `n` autotroph-leaning organisms on `Land` cells that already hold
/// at least `min_valaar` (so they have food). Returns how many were seeded.
pub fn seed_on_fed_land(
    sim: &mut Sim<Grid2p5D>,
    mats: &[CellType],
    n: usize,
    min_valaar: f32,
) -> usize {
    let (sw, sh) = (sim.world.space.width(), sim.world.space.height());
    let mut cells = Vec::new();
    for y in 0..sh {
        for x in 0..sw {
            let c = Coord::new(x, y, Layer::Surface);
            if mats[(y * sw + x) as usize] == CellType::Land
                && sim.world.valaar.get(sim.world.space.index(c)) >= min_valaar
            {
                cells.push(c);
            }
        }
    }
    if cells.is_empty() {
        return 0;
    }
    for _ in 0..n {
        let c = cells[(sim.rng.next_unit() * cells.len() as f32) as usize % cells.len()];
        let g = Genome::from_array([
            0.45,                          // size
            0.7,                           // valaar_efficiency
            0.4,                           // speed
            sim.rng.next_unit() * 0.4,     // diet (autotroph-leaning)
            0.5,                           // repro_threshold
            0.5,                           // lifespan
            0.5,                           // heat_tolerance
            0.5,                           // drought_tolerance
            0.0,                           // swim (arises by mutation)
        ])
        .clamped();
        let energy = sim.eco.initial_energy;
        sim.seed_organism(TraitOrganism::new(g, c, energy));
    }
    n
}

/// Packed `0x00RRGGBB` marker colour: hue by diet (green autotroph → red
/// predator). Brightness is constant — the circle outline carries contrast.
pub fn marker_color(o: &TraitOrganism) -> u32 {
    let r = (o.genome.diet * 255.0) as u32;
    let g = ((1.0 - o.genome.diet) * 255.0) as u32;
    let b = 60u32;
    (r << 16) | (g << 8) | b
}

/// Flood-fill connected components of non-Ocean terrain (4-connectivity) on the
/// `sw×sh` sim plane. Returns a per-cell continent label (`None` for Ocean) and
/// the number of continents. Deterministic: row-major scan, first-seen labels.
pub fn label_continents(mats: &[CellType], sw: u32, sh: u32) -> (Vec<Option<u32>>, u32) {
    let n = (sw * sh) as usize;
    let mut labels: Vec<Option<u32>> = vec![None; n];
    let mut next: u32 = 0;
    for start in 0..n {
        if mats[start] == CellType::Ocean || labels[start].is_some() {
            continue;
        }
        let label = next;
        next += 1;
        labels[start] = Some(label);
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let (x, y) = (i as u32 % sw, i as u32 / sw);
            let mut neighbors = Vec::with_capacity(4);
            if x > 0 {
                neighbors.push(i - 1);
            }
            if x + 1 < sw {
                neighbors.push(i + 1);
            }
            if y > 0 {
                neighbors.push(i - sw as usize);
            }
            if y + 1 < sh {
                neighbors.push(i + sw as usize);
            }
            for j in neighbors {
                if mats[j] != CellType::Ocean && labels[j].is_none() {
                    labels[j] = Some(label);
                    stack.push(j);
                }
            }
        }
    }
    (labels, next)
}

/// Per-continent tally: population and mean body size.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinentStat {
    pub label: u32,
    pub count: usize,
    pub mean_size: f32,
}

/// A snapshot of the living population for the viewer HUD.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub total: usize,
    pub autotrophs: usize,
    pub predators: usize,
    pub mean_size: f32,
    /// Per-continent tally, sorted by population desc then label asc.
    pub continents: Vec<ContinentStat>,
}

/// Tally the population: diet split (`diet <= 0.5` = autotroph-leaning), mean
/// body size, and per-continent population using `labels` from
/// `label_continents`. `sw` is the sim-plane width (to index `labels`).
pub fn compute_stats(
    pop: &Population,
    sw: u32,
    labels: &[Option<u32>],
    n_continents: u32,
) -> Stats {
    let orgs = pop.organisms();
    let total = orgs.len();
    let mut autotrophs = 0usize;
    let mut size_sum = 0.0f32;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::EcoParams;

    fn grid(s: &[&str]) -> (u32, u32, Vec<CellType>) {
        let h = s.len() as u32;
        let w = s[0].len() as u32;
        let mut m = Vec::new();
        for row in s {
            for ch in row.chars() {
                m.push(match ch {
                    'O' => CellType::Ocean,
                    'V' => CellType::Valaar,
                    _ => CellType::Land,
                });
            }
        }
        (w, h, m)
    }

    #[test]
    fn downscale_centre_samples() {
        // 6×3 -> scale 3 -> 2×1; centre of each 3×3 block is (col 1, row 1) / (col 4, row 1)
        let (w, h, m) = grid(&["OOOLLL", "OVOLLL", "OOOLLL"]);
        let (sw, sh, out) = downscale(&m, w, h, 3);
        assert_eq!((sw, sh), (2, 1));
        assert_eq!(out, vec![CellType::Valaar, CellType::Land]); // (1,1)=V, (4,1)=L
    }

    #[test]
    fn world_registers_a_source_per_valaar_cell() {
        let (w, h, m) = grid(&["OVO", "VLV"]);
        let world = world_from_materials(w, h, &m);
        assert_eq!(world.sources().len(), 3); // three V cells
        assert!(world.sources().contains(&Coord::new(1, 0, Layer::Surface)));
    }

    #[test]
    fn seeding_only_lands_on_fed_land() {
        let (w, h, m) = grid(&["VLO"]); // valaar, land, ocean
        let mut sim = Sim::new(world_from_materials(w, h, &m), EcoParams::default(), 1);
        for _ in 0..50 {
            sim.world.step(); // spread valaar onto the land cell
        }
        let seeded = seed_on_fed_land(&mut sim, &m, 10, 0.001);
        assert_eq!(seeded, 10);
        for o in sim.pop.organisms() {
            assert_eq!(o.pos, Coord::new(1, 0, Layer::Surface), "only the land cell is fed");
        }
    }

    #[test]
    fn world_marks_ocean_and_valaar_impassable() {
        let (w, h, m) = grid(&["OVL"]); // ocean, valaar, land
        let world = world_from_materials(w, h, &m);
        let mask = world.passability().expect("mask installed");
        let idx = |x: u32| world.space.index(Coord::new(x, 0, Layer::Surface));
        assert!(!mask[idx(0)], "ocean impassable");
        assert!(!mask[idx(1)], "valaar impassable");
        assert!(mask[idx(2)], "land passable");
    }

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

    #[test]
    fn marker_color_greens_autotrophs_reds_predators() {
        let c = Coord::new(0, 0, Layer::Surface);
        let auto = TraitOrganism::new(
            Genome::from_array([0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.5, 0.5]), c, 1.0);
        let pred = TraitOrganism::new(
            Genome::from_array([0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5]), c, 1.0);
        let green = |p: u32| (p >> 8) & 0xFF;
        let red = |p: u32| (p >> 16) & 0xFF;
        assert!(green(marker_color(&auto)) > red(marker_color(&auto)));
        assert!(red(marker_color(&pred)) > green(marker_color(&pred)));
    }

    #[test]
    fn continents_split_on_ocean_only() {
        let (w, h, m) = grid(&["LOLL"]); // Land | Ocean | Land Land
        let (labels, n) = label_continents(&m, w, h);
        assert_eq!(n, 2);
        assert_eq!(labels[0], Some(0));
        assert_eq!(labels[1], None); // ocean
        assert_eq!(labels[2], Some(1));
        assert_eq!(labels[3], Some(1));
    }

    #[test]
    fn compute_stats_tallies_diet_size_and_continents() {
        let (w, _h, m) = grid(&["LOLL"]);
        let (labels, n) = label_continents(&m, w, 1);
        let mut pop = Population::new();
        let g = |diet: f32, size: f32| {
            Genome::from_array([size, 0.5, 0.5, diet, 0.5, 0.5, 0.5, 0.5, 0.5])
        };
        pop.spawn(TraitOrganism::new(g(0.0, 0.2), Coord::new(0, 0, Layer::Surface), 1.0));
        pop.spawn(TraitOrganism::new(g(0.0, 0.4), Coord::new(0, 0, Layer::Surface), 1.0));
        pop.spawn(TraitOrganism::new(g(1.0, 0.6), Coord::new(2, 0, Layer::Surface), 1.0));

        let s = compute_stats(&pop, w, &labels, n);
        assert_eq!(s.total, 3);
        assert_eq!(s.autotrophs, 2);
        assert_eq!(s.predators, 1);
        assert!((s.mean_size - 0.4).abs() < 1e-6);
        assert_eq!(s.continents.len(), 2);
        assert_eq!((s.continents[0].label, s.continents[0].count), (0, 2));
        assert!((s.continents[0].mean_size - 0.3).abs() < 1e-6); // (0.2 + 0.4) / 2
        assert_eq!((s.continents[1].label, s.continents[1].count), (1, 1));
        assert!((s.continents[1].mean_size - 0.6).abs() < 1e-6);
    }
}
