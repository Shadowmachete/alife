//! Static geography — the abiotic stage. `CellType` names what fills a cell and
//! carries the two properties plan 5's ecology will consume: valaar
//! `conductivity` (how freely valaar diffuses through it) and `passable` (whether
//! an organism may move into it). `TerrainMap` (below) is a grid of these.
//!
//! Authored by `worldgen`, persisted as JSON here, drawn by `viewer`.

use serde::{Deserialize, Serialize};

/// What fills a cell.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CellType {
    Ocean,
    Land,
    River,
    Rock,
    Mountain,
    Rasconne,
}

impl CellType {
    pub const ALL: [CellType; 6] = [
        CellType::Ocean,
        CellType::Land,
        CellType::River,
        CellType::Rock,
        CellType::Mountain,
        CellType::Rasconne,
    ];

    /// How freely valaar diffuses through this cell (0 = blocks). A multiplier on
    /// diffusion, consumed by plan 5; carried here so the map is self-describing.
    /// [ASSUMPTION A2]
    pub fn conductivity(self) -> f32 {
        match self {
            CellType::Ocean => 0.0,
            CellType::Mountain => 0.0,
            CellType::Rock => 0.3,
            CellType::Land => 1.0,
            CellType::River => 1.5,
            CellType::Rasconne => 2.0,
        }
    }

    /// Whether an organism may move into this cell (plan 5 consumes this).
    pub fn passable(self) -> bool {
        !matches!(self, CellType::Ocean | CellType::Mountain)
    }

    /// One-char code for compact text / debugging.
    pub fn code(self) -> char {
        match self {
            CellType::Ocean => 'O',
            CellType::Land => '.',
            CellType::River => '~',
            CellType::Rock => ':',
            CellType::Mountain => '^',
            CellType::Rasconne => 'R',
        }
    }

    pub fn from_code(c: char) -> Option<CellType> {
        match c {
            'O' => Some(CellType::Ocean),
            '.' => Some(CellType::Land),
            '~' => Some(CellType::River),
            ':' => Some(CellType::Rock),
            '^' => Some(CellType::Mountain),
            'R' => Some(CellType::Rasconne),
            _ => None,
        }
    }

    /// Solid colour `[r, g, b]` the viewer paints for this cell (palette per
    /// `docs/tile-design-guide.md`). Replaced by textured tiles later.
    pub fn fallback_rgb(self) -> [u8; 3] {
        match self {
            CellType::Ocean => [20, 28, 64],
            CellType::Land => [96, 120, 72],
            CellType::River => [40, 180, 170],
            CellType::Rock => [96, 92, 110],
            CellType::Mountain => [120, 120, 130],
            CellType::Rasconne => [220, 80, 60],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_round_trips_for_every_type() {
        for t in CellType::ALL {
            assert_eq!(CellType::from_code(t.code()), Some(t), "round-trip {t:?}");
        }
        assert_eq!(CellType::from_code('?'), None);
    }

    #[test]
    fn oceans_and_mountains_block_movement_and_valaar() {
        assert!(!CellType::Ocean.passable());
        assert!(!CellType::Mountain.passable());
        assert_eq!(CellType::Ocean.conductivity(), 0.0);
        assert_eq!(CellType::Mountain.conductivity(), 0.0);
    }

    #[test]
    fn rivers_and_the_core_conduct_better_than_land() {
        assert!(CellType::River.conductivity() > CellType::Land.conductivity());
        assert!(CellType::Rasconne.conductivity() >= CellType::River.conductivity());
        assert!(CellType::Land.passable());
        assert!(CellType::Rasconne.passable());
    }

    #[test]
    fn every_type_has_a_distinct_colour() {
        let mut seen = std::collections::HashSet::new();
        for t in CellType::ALL {
            assert!(seen.insert(t.fallback_rgb()), "duplicate colour for {t:?}");
        }
    }
}
