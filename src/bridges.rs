//! Periodic land bridges: a Vraze-season disturbance that raises temporary land
//! across narrow ocean straits between continents, giving intermittent gene flow
//! (allopatric speciation with reconnection). Sites are found once at load; the
//! `Bridges` controller (Task 2) schedules which open, when, and for how long.

use crate::rng::Rng;
use crate::terrain::CellType;

/// Tunable dials for bridge generation and scheduling.
#[derive(Clone, Copy, Debug)]
pub struct BridgeConfig {
    /// Longest ocean span (cells) that can become a bridge.
    pub max_gap: u32,
    /// Ribbon width range (cells), inclusive.
    pub min_width: u32,
    pub max_width: u32,
    /// Fraction of candidate straits kept as sites at load.
    pub site_fraction: f32,
    /// Fraction of sites that open in any given Vraze.
    pub open_fraction: f32,
    /// A bridge stays open this many craws (range, clamped into the Vraze arh).
    pub min_duration: u32,
    pub max_duration: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        BridgeConfig {
            max_gap: 8,
            min_width: 3,
            max_width: 6,
            site_fraction: 0.5,
            open_fraction: 0.5,
            min_duration: 20,
            max_duration: 80,
        }
    }
}

/// One bridge corridor: the surface-plane cell indices (`y*sw + x`) of its ocean
/// ribbon. Flipping these passable opens the bridge; reverting closes it.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeSite {
    pub cells: Vec<usize>,
}

/// Find candidate bridge corridors and keep a seeded-random subset. A candidate
/// is a straight run of Ocean cells (≤ `cfg.max_gap` long) flanked at both ends
/// by land of two *different* continents; it is widened perpendicular into a
/// ribbon `cfg.min_width..=cfg.max_width` cells across (Ocean cells only). Cells
/// are claimed so sites never overlap. Deterministic over `(mats, labels, rng,
/// cfg)` with a fixed horizontal-then-vertical, row-major scan.
pub fn find_bridge_sites(
    mats: &[CellType],
    sw: u32,
    sh: u32,
    labels: &[Option<u32>],
    rng: &mut Rng,
    cfg: &BridgeConfig,
) -> Vec<BridgeSite> {
    let idx = |x: u32, y: u32| (y * sw + x) as usize;
    let is_ocean = |x: u32, y: u32| mats[idx(x, y)] == CellType::Ocean;

    // (run_cells, widen_along_y): a horizontal strait widens along y; vertical along x.
    let mut candidates: Vec<(Vec<usize>, bool)> = Vec::new();

    // Horizontal straits: ocean runs along x at a fixed row, land on left/right.
    for y in 0..sh {
        let mut x = 0u32;
        while x < sw {
            if !is_ocean(x, y) {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < sw && is_ocean(x, y) {
                x += 1;
            }
            let x1 = x - 1;
            let len = x1 - x0 + 1;
            if x0 == 0 || x1 + 1 >= sw || len > cfg.max_gap {
                continue;
            }
            if let (Some(a), Some(b)) = (labels[idx(x0 - 1, y)], labels[idx(x1 + 1, y)]) {
                if a != b {
                    candidates.push(((x0..=x1).map(|xx| idx(xx, y)).collect(), true));
                }
            }
        }
    }

    // Vertical straits: ocean runs along y at a fixed column, land on top/bottom.
    for x in 0..sw {
        let mut y = 0u32;
        while y < sh {
            if !is_ocean(x, y) {
                y += 1;
                continue;
            }
            let y0 = y;
            while y < sh && is_ocean(x, y) {
                y += 1;
            }
            let y1 = y - 1;
            let len = y1 - y0 + 1;
            if y0 == 0 || y1 + 1 >= sh || len > cfg.max_gap {
                continue;
            }
            if let (Some(a), Some(b)) = (labels[idx(x, y0 - 1)], labels[idx(x, y1 + 1)]) {
                if a != b {
                    candidates.push(((y0..=y1).map(|yy| idx(x, yy)).collect(), false));
                }
            }
        }
    }

    let mut claimed = vec![false; (sw * sh) as usize];
    let mut sites = Vec::new();
    for (run, widen_y) in candidates {
        if rng.next_unit() >= cfg.site_fraction {
            continue; // keep a random subset
        }
        if run.iter().any(|&c| claimed[c]) {
            continue; // overlaps an already-carved site
        }
        let span = (cfg.max_width.saturating_sub(cfg.min_width) + 1) as f32;
        let width = (cfg.min_width + (rng.next_unit() * span) as u32).clamp(cfg.min_width, cfg.max_width);
        let mut cells = Vec::new();
        for &c in &run {
            let cx = (c as u32) % sw;
            let cy = (c as u32) / sw;
            for w in 0..width {
                let (nx, ny) = if widen_y { (cx, cy + w) } else { (cx + w, cy) };
                if nx >= sw || ny >= sh {
                    break;
                }
                let ni = idx(nx, ny);
                if mats[ni] == CellType::Ocean && !claimed[ni] {
                    claimed[ni] = true;
                    cells.push(ni);
                }
            }
        }
        if !cells.is_empty() {
            cells.sort_unstable();
            sites.push(BridgeSite { cells });
        }
    }
    sites
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapsim::label_continents;

    // 'O' -> Ocean, else Land. Returns (sw, sh, mats).
    fn grid(rows: &[&str]) -> (u32, u32, Vec<CellType>) {
        let sh = rows.len() as u32;
        let sw = rows[0].len() as u32;
        let mut mats = Vec::with_capacity((sw * sh) as usize);
        for r in rows {
            for ch in r.chars() {
                mats.push(if ch == 'O' { CellType::Ocean } else { CellType::Land });
            }
        }
        (sw, sh, mats)
    }

    fn cfg_keep_all(min_width: u32, max_width: u32, max_gap: u32) -> BridgeConfig {
        BridgeConfig { max_gap, min_width, max_width, site_fraction: 1.0, ..BridgeConfig::default() }
    }

    #[test]
    fn finds_a_strait_between_two_continents() {
        let (sw, sh, m) = grid(&["LLOOLL"]); // cont0 | ocean | cont1
        let (labels, n) = label_continents(&m, sw, sh);
        assert_eq!(n, 2);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(1, 1, 4));
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].cells, vec![2, 3]); // the two ocean cells
    }

    #[test]
    fn ignores_gaps_wider_than_max_gap() {
        let (sw, sh, m) = grid(&["LLOOOOOOLL"]); // 6-wide ocean
        let (labels, _) = label_continents(&m, sw, sh);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(1, 1, 4));
        assert!(sites.is_empty(), "gap of 6 > max_gap 4");
    }

    #[test]
    fn ignores_ocean_within_one_continent() {
        let (sw, sh, m) = grid(&["LOL", "LLL"]); // the L's all connect around the lake
        let (labels, n) = label_continents(&m, sw, sh);
        assert_eq!(n, 1);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(1, 1, 4));
        assert!(sites.is_empty(), "same continent on both sides -> not a strait");
    }

    #[test]
    fn widens_into_a_ribbon_and_claims_cells() {
        let (sw, sh, m) = grid(&["LLOOLL", "LLOOLL", "LLOOLL"]);
        let (labels, _) = label_continents(&m, sw, sh);
        let mut rng = Rng::new(1);
        let sites = find_bridge_sites(&m, sw, sh, &labels, &mut rng, &cfg_keep_all(3, 3, 4));
        assert_eq!(sites.len(), 1, "rows after the first reuse claimed cells");
        assert_eq!(sites[0].cells.len(), 6, "3 rows x 2 ocean columns");
    }

    #[test]
    fn is_seed_deterministic() {
        let (sw, sh, m) = grid(&["LLOOLL", "LLOOLL"]);
        let (labels, _) = label_continents(&m, sw, sh);
        let a = find_bridge_sites(&m, sw, sh, &labels, &mut Rng::new(9), &BridgeConfig::default());
        let b = find_bridge_sites(&m, sw, sh, &labels, &mut Rng::new(9), &BridgeConfig::default());
        assert_eq!(a, b);
    }
}
