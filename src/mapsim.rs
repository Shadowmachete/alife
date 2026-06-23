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
    let mean_size = if total > 0 {
        size_sum / total as f32
    } else {
        0.0
    };
    let mut continents: Vec<ContinentStat> = (0..n_continents as usize)
        .map(|l| ContinentStat {
            label: l as u32,
            count: per_count[l],
            mean_size: if per_count[l] > 0 {
                per_size[l] / per_count[l] as f32
            } else {
                0.0
            },
        })
        .collect();
    continents.sort_by(|a, b| b.count.cmp(&a.count).then(a.label.cmp(&b.label)));
    Stats {
        total,
        autotrophs,
        predators: total - autotrophs,
        mean_size,
        continents,
    }
}

/// Sparse (north) edge stride of a reservoir gradient region — one column per
/// this many cells where the gradient begins. [D6]
const RESERVOIR_MAX_STRIDE: i64 = 8;
/// Dense (south) edge stride. `1` lines the southmost row solidly. [D6]
const RESERVOIR_MIN_STRIDE: i64 = 3;
/// Fraction of a continent's latitude span (measured from the south) the
/// reservoir gradient covers; the northern remainder stays empty. [D6]
const RESERVOIR_BAND_FRAC: f32 = 0.5;

/// The southmost cell of continent `target` (ties broken to the **west**), or
/// `None` if the label is absent. Guarantees the southern tip always holds a
/// reservoir even when the gradient strides skip the exact corner. [D6]
fn south_tip(sw: u32, sh: u32, labels: &[Option<u32>], target: u32) -> Option<(u32, u32)> {
    let idx = |x: u32, y: u32| (y * sw + x) as usize;
    let mut best: Option<(u32, u32)> = None;
    let mut score = i64::MIN;
    for y in 0..sh {
        for x in 0..sw {
            if labels[idx(x, y)] == Some(target) {
                let s = y as i64 * sw as i64 - x as i64; // southmost, tie -> westmost
                if s > score {
                    score = s;
                    best = Some((x, y));
                }
            }
        }
    }
    best
}

/// Scatter reservoir columns over continent `target`'s **southern band** as a
/// south-ward density gradient: sparse at the band's north edge, dense (the full
/// row) at the south. `west_only` confines the spread to the continent's western
/// half — the major continent's south-west quadrant. Deterministic, no RNG. [D6]
fn gradient_columns(
    sw: u32,
    sh: u32,
    labels: &[Option<u32>],
    target: u32,
    west_only: bool,
) -> Vec<(u32, u32)> {
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
    let mut cols = Vec::new();
    if !any {
        return cols;
    }
    let span = (y_max - y_min) as f32;
    let y_top = y_max - (span * RESERVOIR_BAND_FRAC).round() as u32;
    let x_limit = if west_only {
        x_min + (x_max - x_min) / 2
    } else {
        x_max
    };
    let denom = (y_max - y_top).max(1) as f32;
    for y in y_top..=y_max {
        let t = (y - y_top) as f32 / denom; // 0 at the band's north, 1 at the south edge
        let stride = (RESERVOIR_MAX_STRIDE as f32
            + (RESERVOIR_MIN_STRIDE - RESERVOIR_MAX_STRIDE) as f32 * t)
            .round()
            .max(1.0) as u32;
        for x in x_min..=x_limit {
            if labels[idx(x, y)] == Some(target) && (x - x_min) % stride == 0 {
                cols.push((x, y));
            }
        }
    }
    cols
}

/// Choose the underground reservoir columns: a south-ward density **gradient**
/// over the **south-west quadrant** of the major (largest) continent and a
/// full-width gradient over the **southmost** other island — sparse to the north,
/// dense at each southern tip. Returns surface-plane `(x, y)` columns (sorted,
/// de-duplicated) to register as underground sources + descent access points.
/// Pure function of the continent labels — deterministic, no RNG. [D6]
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
    let mut cols = Vec::new();
    // Major continent: a south-west-quadrant gradient + a guaranteed tip.
    cols.extend(gradient_columns(sw, sh, labels, major, true));
    cols.extend(south_tip(sw, sh, labels, major));
    // Southmost island (if any): a full-width gradient + a guaranteed tip.
    if let Some(isl) = island {
        cols.extend(gradient_columns(sw, sh, labels, isl, false));
        cols.extend(south_tip(sw, sh, labels, isl));
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

    /// Two stacked all-land continents separated by `gap` ocean rows: a `top_h`
    /// continent (label 0) and a `bottom_h` island (label 1) to its south.
    /// Returns `(w, h, labels)`.
    fn two_continents(w: u32, top_h: u32, gap: u32, bottom_h: u32) -> (u32, u32, Vec<Option<u32>>) {
        let h = top_h + gap + bottom_h;
        let mut m = vec![CellType::Land; (w * h) as usize];
        for y in top_h..top_h + gap {
            for x in 0..w {
                m[(y * w + x) as usize] = CellType::Ocean;
            }
        }
        let (labels, _n) = label_continents(&m, w, h);
        (w, h, labels)
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
            assert_eq!(
                o.pos,
                Coord::new(1, 0, Layer::Surface),
                "only the land cell is fed"
            );
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
            Genome::from_array([0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]),
            c,
            1.0,
        );
        let pred = TraitOrganism::new(
            Genome::from_array([0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]),
            c,
            1.0,
        );
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
            Genome::from_array([size, 0.5, 0.5, diet, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5])
        };
        pop.spawn(TraitOrganism::new(
            g(0.0, 0.2),
            Coord::new(0, 0, Layer::Surface),
            1.0,
        ));
        pop.spawn(TraitOrganism::new(
            g(0.0, 0.4),
            Coord::new(0, 0, Layer::Surface),
            1.0,
        ));
        pop.spawn(TraitOrganism::new(
            g(1.0, 0.6),
            Coord::new(2, 0, Layer::Surface),
            1.0,
        ));

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

    #[test]
    fn reservoirs_sit_at_major_sw_tip_and_southmost_island() {
        // A big 3x3 continent (top-left) and a one-cell island at the bottom-right.
        let (w, h, m) = grid(&["LLLO", "LLLO", "LLLO", "OOOL"]);
        let (labels, _n) = label_continents(&m, w, h);
        let cols = place_underground_reservoirs(w, h, &labels);
        // SW tip of the major continent is around (0,2); island is the (3,3) cell.
        assert!(
            cols.contains(&(0, 2)),
            "expected a reservoir at the major SW tip, got {cols:?}"
        );
        assert!(
            cols.contains(&(3, 3)),
            "expected a reservoir on the southmost island, got {cols:?}"
        );
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
        assert!(
            cols.iter().all(|&(x, y)| x <= 1 && y <= 1),
            "clustered at the SW tip, got {cols:?}"
        );
    }

    #[test]
    fn reservoirs_spread_across_the_south_not_one_blob() {
        let (w, h) = (12u32, 12u32);
        let labels = vec![Some(0u32); (w * h) as usize];
        let cols = place_underground_reservoirs(w, h, &labels);
        let xs: Vec<u32> = cols.iter().map(|c| c.0).collect();
        let ys: Vec<u32> = cols.iter().map(|c| c.1).collect();
        let span_x = xs.iter().max().unwrap() - xs.iter().min().unwrap();
        let span_y = ys.iter().max().unwrap() - ys.iter().min().unwrap();
        assert!(
            span_x > 2 && span_y > 2,
            "reservoirs should spread out, not sit in one 3x3 blob: {cols:?}"
        );
        assert!(
            cols.len() >= 8,
            "expected many reservoirs, got {}: {cols:?}",
            cols.len()
        );
    }

    #[test]
    fn reservoir_density_increases_toward_the_south() {
        let (w, h) = (12u32, 12u32);
        let labels = vec![Some(0u32); (w * h) as usize];
        let cols = place_underground_reservoirs(w, h, &labels);
        let mut rows: Vec<u32> = cols.iter().map(|c| c.1).collect();
        rows.sort_unstable();
        rows.dedup();
        assert!(
            rows.len() > 2,
            "reservoirs should occupy a gradient of rows, got {rows:?}"
        );
        let (north_edge, south_edge) = (*rows.first().unwrap(), *rows.last().unwrap());
        let at = |row: u32| cols.iter().filter(|c| c.1 == row).count();
        assert!(
            at(south_edge) > at(north_edge),
            "south edge (row {south_edge}) should be denser than the north edge (row {north_edge}): {cols:?}"
        );
    }

    #[test]
    fn major_continent_reservoirs_fill_the_south_west_quadrant() {
        let (w, h) = (12u32, 12u32);
        let labels = vec![Some(0u32); (w * h) as usize];
        let cols = place_underground_reservoirs(w, h, &labels);
        assert!(
            cols.iter()
                .all(|&(x, y)| x <= w / 2 && y as i32 >= h as i32 / 2 - 1),
            "reservoirs should stay in the south-west quadrant: {cols:?}"
        );
        let min_y = cols.iter().map(|c| c.1).min().unwrap();
        assert!(
            min_y <= h / 2 + 1,
            "reservoirs should reach the north of the region, min_y={min_y}"
        );
    }

    #[test]
    fn island_reservoirs_spread_with_a_dense_south_tip() {
        // Major continent (12 rows), an ocean divider, then an 8-row island.
        let (w, h, labels) = two_continents(10, 12, 1, 8);
        let island_start = 12 + 1; // first land row of the island
        let cols = place_underground_reservoirs(w, h, &labels);
        let island: Vec<(u32, u32)> = cols.into_iter().filter(|c| c.1 >= island_start).collect();
        assert!(
            island.len() >= 6,
            "island should hold many reservoirs, got {}: {island:?}",
            island.len()
        );
        let xs: Vec<u32> = island.iter().map(|c| c.0).collect();
        let ys: Vec<u32> = island.iter().map(|c| c.1).collect();
        let span_x = xs.iter().max().unwrap() - xs.iter().min().unwrap();
        let span_y = ys.iter().max().unwrap() - ys.iter().min().unwrap();
        assert!(
            span_x > 2 && span_y > 2,
            "island reservoirs should spread, not a blob: {island:?}"
        );
        assert!(
            island.iter().any(|&(_, y)| y == h - 1),
            "the south tip row should hold reservoirs: {island:?}"
        );
    }

    #[test]
    fn add_reservoirs_registers_sources_access_points_and_descendable() {
        let (w, h, m) = grid(&["LL", "LL"]);
        let mut world = world_from_materials(w, h, &m);
        add_underground_reservoirs(&mut world, &[(1, 1)]);
        assert!(world
            .sources()
            .contains(&Coord::new(1, 1, Layer::Underground)));
        assert!(world.access_points().contains(&(1, 1)));
        let desc = world.descendable().expect("descendable installed");
        assert!(desc[world.space.index(Coord::new(1, 1, Layer::Surface))]);
        assert!(desc[world.space.index(Coord::new(1, 1, Layer::Underground))]);
    }
}
