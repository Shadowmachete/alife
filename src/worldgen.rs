//! The lore-constrained, deterministic world generator. It takes a coarse
//! `Sketch` and fleshes it into a full-resolution `TerrainMap`: upscale the
//! sketch, stamp a central Rasconne core, (Task 6) trace valaar rivers outward
//! and fade the periphery into the Dusk, and fill the underground placeholder.
//!
//! Pure function of `(sketch, width, height, seed)` — the only randomness is an
//! `Rng` seeded from `seed`, so the canonical `ALCHAEA_SEED` always rebuilds the
//! same world.

use crate::rng::Rng;
use crate::sketch::Sketch;
use crate::space::{Coord, Grid2p5D, Layer, Space};
use crate::terrain::{CellType, TerrainMap};

/// The canonical seed — "the" Alchaea.
pub const ALCHAEA_SEED: u64 = 0x00A1_C4EA;

/// Radius (cells) of the Rasconne core stamped at the map centre. [A3]
const RASCONNE_RADIUS: i32 = 2;
/// Number of valaar rivers traced from the core. [A4]
const RIVER_COUNT: u32 = 6;
/// Maximum length (steps) of each river. [A4]
const RIVER_MAX_LEN: u32 = 600;
/// Decorrelates the river RNG from any other use of the seed.
const RIVER_SALT: u64 = 0x5249_5645_52; // "RIVER"
/// Surface land beyond this fraction of the half-min-dimension becomes Dusk rock. [A5]
const DUSK_FRACTION: f32 = 0.55;

/// Generate a full-resolution terrain map from a coarse sketch.
pub fn generate(sketch: &Sketch, width: u32, height: u32, seed: u64) -> TerrainMap {
    let space = Grid2p5D::new(width, height);
    let mut map = TerrainMap::filled(space.len(), width, height, CellType::Ocean, seed);
    upscale_surface(&space, &mut map, sketch);
    let center = (width / 2, height / 2);
    stamp_rasconne(&space, &mut map, center);
    trace_rivers(&space, &mut map, center, seed);
    mark_dusk(&space, &mut map, center);
    fill_underground(&space, &mut map);
    map
}

/// Blow the coarse sketch up to full resolution by nearest-coarse-cell.
fn upscale_surface<S: Space>(space: &S, map: &mut TerrainMap, sketch: &Sketch) {
    let (w, h) = (map.width(), map.height());
    let (cw, ch) = (sketch.width.max(1), sketch.height.max(1));
    for y in 0..h {
        for x in 0..w {
            let cx = (x * cw / w).min(cw - 1);
            let cy = (y * ch / h).min(ch - 1);
            let region = sketch.regions[(cy * cw + cx) as usize];
            map.set(space.index(Coord::new(x, y, Layer::Surface)), region);
        }
    }
}

/// Stamp a Rasconne disk at the map centre, enforcing the central-core lore. [A3]
fn stamp_rasconne<S: Space>(space: &S, map: &mut TerrainMap, center: (u32, u32)) {
    let (cx, cy) = (center.0 as i32, center.1 as i32);
    let (w, h) = (map.width() as i32, map.height() as i32);
    for dy in -RASCONNE_RADIUS..=RASCONNE_RADIUS {
        for dx in -RASCONNE_RADIUS..=RASCONNE_RADIUS {
            if dx * dx + dy * dy > RASCONNE_RADIUS * RASCONNE_RADIUS {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            map.set(
                space.index(Coord::new(x as u32, y as u32, Layer::Surface)),
                CellType::Rasconne,
            );
        }
    }
}

/// Fill the underground layer with a uniform placeholder. [A6]
fn fill_underground<S: Space>(space: &S, map: &mut TerrainMap) {
    let (w, h) = (map.width(), map.height());
    for y in 0..h {
        for x in 0..w {
            map.set(space.index(Coord::new(x, y, Layer::Underground)), CellType::Rock);
        }
    }
}

/// Trace seeded, gently-meandering rivers from the core outward. Each stops at
/// ocean, a mountain, or the edge; rivers flow through (not over) Rasconne. [A4]
fn trace_rivers<S: Space>(space: &S, map: &mut TerrainMap, center: (u32, u32), seed: u64) {
    let mut rng = Rng::new(seed ^ RIVER_SALT);
    let (w, h) = (map.width() as i32, map.height() as i32);
    for _ in 0..RIVER_COUNT {
        let mut angle = rng.next_unit() * std::f32::consts::TAU;
        let mut fx = center.0 as f32;
        let mut fy = center.1 as f32;
        for _ in 0..RIVER_MAX_LEN {
            angle += (rng.next_unit() - 0.5) * 0.5; // gentle meander
            fx += angle.cos();
            fy += angle.sin();
            let (x, y) = (fx.round() as i32, fy.round() as i32);
            if x < 0 || y < 0 || x >= w || y >= h {
                break;
            }
            let i = space.index(Coord::new(x as u32, y as u32, Layer::Surface));
            match map.get(i) {
                CellType::Ocean | CellType::Mountain => break,
                CellType::Rasconne => continue,
                _ => map.set(i, CellType::River),
            }
        }
    }
}

/// Fade surface `Land` beyond the Dusk radius into `Rock` — the oligotrophic
/// periphery. Rivers, ocean, mountains, and the core are left alone. [A5]
fn mark_dusk<S: Space>(space: &S, map: &mut TerrainMap, center: (u32, u32)) {
    let (w, h) = (map.width(), map.height());
    let threshold = DUSK_FRACTION * (w.min(h) as f32) * 0.5;
    for y in 0..h {
        for x in 0..w {
            let i = space.index(Coord::new(x, y, Layer::Surface));
            if map.get(i) != CellType::Land {
                continue;
            }
            let dx = x as f32 - center.0 as f32;
            let dy = y as f32 - center.1 as f32;
            if (dx * dx + dy * dy).sqrt() > threshold {
                map.set(i, CellType::Rock);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::Sketch;
    use crate::space::{Coord, Grid2p5D, Layer, Space};
    use crate::terrain::CellType;

    // A coarse sketch: ocean border, land interior.
    fn land_in_ocean(cw: u32, ch: u32) -> Sketch {
        let mut regions = vec![CellType::Land; (cw * ch) as usize];
        for x in 0..cw {
            regions[x as usize] = CellType::Ocean;
            regions[((ch - 1) * cw + x) as usize] = CellType::Ocean;
        }
        for y in 0..ch {
            regions[(y * cw) as usize] = CellType::Ocean;
            regions[(y * cw + cw - 1) as usize] = CellType::Ocean;
        }
        Sketch { width: cw, height: ch, regions }
    }

    #[test]
    fn map_is_sized_to_the_space_with_both_layers() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(map.len(), space.len());
        assert_eq!(map.width(), 24);
        assert_eq!(map.height(), 24);
        assert_eq!(map.layers(), 2);
    }

    #[test]
    fn rasconne_sits_at_the_centre() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(map.get(space.index(Coord::new(12, 12, Layer::Surface))), CellType::Rasconne);
    }

    #[test]
    fn ocean_border_survives_upscaling() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(map.get(space.index(Coord::new(0, 0, Layer::Surface))), CellType::Ocean);
    }

    #[test]
    fn underground_is_a_uniform_placeholder() {
        let space = Grid2p5D::new(24, 24);
        let map = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        for x in 0..24 {
            for y in 0..24 {
                assert_eq!(map.get(space.index(Coord::new(x, y, Layer::Underground))), CellType::Rock);
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        let b = generate(&land_in_ocean(6, 6), 24, 24, ALCHAEA_SEED);
        assert_eq!(a, b, "same sketch + seed must give an identical map");
    }

    fn surface_counts(map: &TerrainMap, space: &Grid2p5D) -> std::collections::HashMap<CellType, u32> {
        let mut counts = std::collections::HashMap::new();
        for y in 0..map.height() {
            for x in 0..map.width() {
                let t = map.get(space.index(Coord::new(x, y, Layer::Surface)));
                *counts.entry(t).or_insert(0) += 1;
            }
        }
        counts
    }

    fn all_land(cw: u32, ch: u32) -> Sketch {
        Sketch { width: cw, height: ch, regions: vec![CellType::Land; (cw * ch) as usize] }
    }

    #[test]
    fn rivers_and_dusk_appear_on_an_all_land_continent() {
        let space = Grid2p5D::new(40, 40);
        let map = generate(&all_land(4, 4), 40, 40, ALCHAEA_SEED);
        let counts = surface_counts(&map, &space);
        assert!(counts.get(&CellType::River).copied().unwrap_or(0) > 0, "rivers should be traced");
        assert!(counts.get(&CellType::Rock).copied().unwrap_or(0) > 0, "the Dusk should appear at the edge");
        assert!(counts.get(&CellType::Land).copied().unwrap_or(0) > 0, "the core ring stays land");
    }

    #[test]
    fn rivers_never_overwrite_the_ocean() {
        let space = Grid2p5D::new(40, 40);
        let mut bordered = vec![CellType::Land; 16];
        for k in 0..4 {
            bordered[k] = CellType::Ocean;
            bordered[12 + k] = CellType::Ocean;
            bordered[k * 4] = CellType::Ocean;
            bordered[k * 4 + 3] = CellType::Ocean;
        }
        let sketch = Sketch { width: 4, height: 4, regions: bordered };
        let map = generate(&sketch, 40, 40, ALCHAEA_SEED);
        assert_eq!(map.get(space.index(Coord::new(0, 0, Layer::Surface))), CellType::Ocean);
    }
}
