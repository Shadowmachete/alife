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
    Valaar,
    Rock,
    Mountain,
}

impl CellType {
    pub const ALL: [CellType; 5] = [
        CellType::Ocean,
        CellType::Land,
        CellType::Valaar,
        CellType::Rock,
        CellType::Mountain,
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
            CellType::Valaar => 2.0,
        }
    }

    /// Whether an organism may move into this cell (plan 5 consumes this).
    pub fn passable(self) -> bool {
        !matches!(
            self,
            CellType::Ocean | CellType::Mountain | CellType::Valaar
        )
    }

    /// One-char code for compact text / debugging.
    pub fn code(self) -> char {
        match self {
            CellType::Ocean => 'O',
            CellType::Land => '.',
            CellType::Valaar => 'V',
            CellType::Rock => ':',
            CellType::Mountain => '^',
        }
    }

    pub fn from_code(c: char) -> Option<CellType> {
        match c {
            'O' => Some(CellType::Ocean),
            '.' => Some(CellType::Land),
            'V' => Some(CellType::Valaar),
            ':' => Some(CellType::Rock),
            '^' => Some(CellType::Mountain),
            _ => None,
        }
    }

    /// Solid colour `[r, g, b]` the viewer paints for this cell (palette per
    /// `docs/tile-design-guide.md`). Replaced by textured tiles later.
    pub fn fallback_rgb(self) -> [u8; 3] {
        match self {
            CellType::Ocean => [20, 28, 64],
            CellType::Land => [96, 120, 72],
            CellType::Valaar => [220, 80, 60],
            CellType::Rock => [96, 92, 110],
            CellType::Mountain => [120, 120, 130],
        }
    }
}

/// A `CellType` per cell, sized to a `Space` (all layers), plus the metadata
/// needed to persist and render it standalone. Parallels `field::Field`: a flat
/// vector indexed via `Space::index`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainMap {
    #[serde(rename = "w")]
    width: u32,
    #[serde(rename = "h")]
    height: u32,
    layers: u32,
    seed: u64,
    cells: Vec<CellType>,
}

impl TerrainMap {
    /// A map of `len` cells (= `space.len()`) all set to `fill`.
    pub fn filled(len: usize, width: u32, height: u32, fill: CellType, seed: u64) -> Self {
        let plane = width as usize * height as usize;
        let layers = if plane == 0 { 0 } else { (len / plane) as u32 };
        TerrainMap {
            width,
            height,
            layers,
            seed,
            cells: vec![fill; len],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn layers(&self) -> u32 {
        self.layers
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn get(&self, i: usize) -> CellType {
        self.cells[i]
    }
    pub fn set(&mut self, i: usize, t: CellType) {
        self.cells[i] = t;
    }
    pub fn cells(&self) -> &[CellType] {
        &self.cells
    }
}

use std::io;
use std::path::Path;

/// Write `map` as JSON (`{w, h, layers, seed, cells:[...]}`).
pub fn save_json(map: &TerrainMap, path: &Path) -> io::Result<()> {
    let json =
        serde_json::to_string(map).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

/// Read a `TerrainMap` back from a JSON file written by `save_json`.
pub fn load_json(path: &Path) -> io::Result<TerrainMap> {
    let s = std::fs::read_to_string(path)?;
    serde_json::from_str(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
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
    fn ocean_mountain_and_valaar_block_movement() {
        assert!(!CellType::Ocean.passable());
        assert!(!CellType::Mountain.passable());
        assert!(!CellType::Valaar.passable());
        assert_eq!(CellType::Ocean.conductivity(), 0.0);
        assert_eq!(CellType::Mountain.conductivity(), 0.0);
    }

    #[test]
    fn valaar_conducts_better_than_land() {
        assert!(CellType::Valaar.conductivity() > CellType::Land.conductivity());
        assert!(CellType::Land.passable());
        assert!(!CellType::Valaar.passable()); // rivers/reservoir: life lives on the banks
    }

    #[test]
    fn every_type_has_a_distinct_colour() {
        let mut seen = std::collections::HashSet::new();
        for t in CellType::ALL {
            assert!(seen.insert(t.fallback_rgb()), "duplicate colour for {t:?}");
        }
    }

    use crate::space::{Coord, Grid2p5D, Layer, Space};

    #[test]
    fn filled_map_is_uniform_and_sized_to_space() {
        let space = Grid2p5D::new(4, 3);
        let map = TerrainMap::filled(space.len(), 4, 3, CellType::Ocean, 42);
        assert_eq!(map.len(), space.len());
        assert_eq!(map.width(), 4);
        assert_eq!(map.height(), 3);
        assert_eq!(map.layers(), 2);
        assert_eq!(map.seed(), 42);
        for i in 0..map.len() {
            assert_eq!(map.get(i), CellType::Ocean);
        }
    }

    #[test]
    fn set_get_round_trips_through_space_index() {
        let space = Grid2p5D::new(4, 3);
        let mut map = TerrainMap::filled(space.len(), 4, 3, CellType::Ocean, 0);
        let c = Coord::new(2, 1, Layer::Surface);
        map.set(space.index(c), CellType::Valaar);
        assert_eq!(map.get(space.index(c)), CellType::Valaar);
        assert_eq!(
            map.get(space.index(Coord::new(0, 0, Layer::Underground))),
            CellType::Ocean
        );
    }

    #[test]
    fn json_round_trips_through_a_file() {
        let space = Grid2p5D::new(5, 4);
        let mut map = TerrainMap::filled(space.len(), 5, 4, CellType::Land, 0xABCD);
        map.set(
            space.index(Coord::new(2, 2, Layer::Surface)),
            CellType::Valaar,
        );
        map.set(
            space.index(Coord::new(0, 0, Layer::Surface)),
            CellType::Ocean,
        );

        let path = std::env::temp_dir().join("alife_terrain_roundtrip.json");
        save_json(&map, &path).unwrap();
        let loaded = load_json(&path).unwrap();
        assert_eq!(loaded, map);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_uses_lowercase_type_names() {
        let space = Grid2p5D::new(1, 1);
        let map = TerrainMap::filled(space.len(), 1, 1, CellType::Ocean, 0);
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains("\"ocean\""), "got {json}");
        assert!(json.contains("\"w\":1"));
    }
}
