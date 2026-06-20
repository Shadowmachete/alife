//! Bridge from a drawn material map to a runnable [`Sim`]: downscale the display
//! grid to a coarser sim grid, seed the valaar field from the map's `Valaar`
//! cells, and seed a population on warmed land.
//!
//! This is the wiring that connects the authored map to the substrate. (Terrain
//! `conductivity`/`passability` are not yet fed into diffusion/movement — a
//! follow-up; for now organisms are simply seeded on fed land and follow the
//! valaar gradient, which keeps them off the foodless ocean.)

use crate::genome::Genome;
use crate::organism::{Organism, TraitOrganism};
use crate::params::EcoParams;
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
/// source — so valaar flows out of the drawn reservoir and rivers.
pub fn world_from_materials(sw: u32, sh: u32, mats: &[CellType]) -> World<Grid2p5D> {
    let mut world = World::new(Grid2p5D::new(sw, sh), Params::default());
    for y in 0..sh {
        for x in 0..sw {
            if mats[(y * sw + x) as usize] == CellType::Valaar {
                world.add_source(Coord::new(x, y, Layer::Surface));
            }
        }
    }
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
        ])
        .clamped();
        let energy = sim.eco.initial_energy;
        sim.seed_organism(TraitOrganism::new(g, c, energy));
    }
    n
}

/// Packed `0x00RRGGBB` dot colour: hue by diet (green autotroph → red predator),
/// brightness by stored-energy fraction.
pub fn dot_color(o: &TraitOrganism, eco: &EcoParams) -> u32 {
    let frac = (o.energy / o.max_energy(eco)).clamp(0.2, 1.0);
    let r = (o.genome.diet * 255.0 * frac) as u32;
    let g = ((1.0 - o.genome.diet) * 255.0 * frac) as u32;
    let b = (70.0 * frac) as u32;
    (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
