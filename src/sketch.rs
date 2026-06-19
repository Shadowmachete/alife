//! Read a hand-drawn map sketch (a Tiled JSON export) into a coarse grid of
//! `CellType` "region seeds" that `worldgen` upscales and fleshes out.
//!
//! Contract [A7]: the sketch's tileset places tiles so global IDs 1..=6 map to
//! `CellType::ALL` order; GID 0/unknown ⇒ Ocean. We read the first tile layer.

use crate::terrain::CellType;
use serde::Deserialize;

/// A coarse map of region seeds, row-major (`width × height`).
pub struct Sketch {
    pub width: u32,
    pub height: u32,
    pub regions: Vec<CellType>,
}

#[derive(Deserialize)]
struct TiledMap {
    layers: Vec<TiledLayer>,
}

#[derive(Deserialize)]
struct TiledLayer {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    data: Vec<u32>,
}

/// Map a Tiled global tile ID to a region seed (flip-flag bits masked off).
fn region_from_gid(gid: u32) -> CellType {
    match gid & 0x1FFF_FFFF {
        1 => CellType::Ocean,
        2 => CellType::Land,
        3 => CellType::River,
        4 => CellType::Rock,
        5 => CellType::Mountain,
        6 => CellType::Rasconne,
        _ => CellType::Ocean,
    }
}

pub fn parse_sketch(json: &str) -> Result<Sketch, String> {
    let map: TiledMap = serde_json::from_str(json).map_err(|e| format!("bad Tiled JSON: {e}"))?;
    let layer = map
        .layers
        .iter()
        .find(|l| l.kind == "tilelayer" && !l.data.is_empty())
        .ok_or_else(|| "no non-empty tile layer found".to_string())?;
    if (layer.width as usize * layer.height as usize) != layer.data.len() {
        return Err(format!(
            "layer size {}x{} does not match {} tiles",
            layer.width,
            layer.height,
            layer.data.len()
        ));
    }
    let regions = layer.data.iter().map(|&gid| region_from_gid(gid)).collect();
    Ok(Sketch { width: layer.width, height: layer.height, regions })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::CellType;

    const FIXTURE: &str = r#"{
        "width": 3, "height": 3,
        "layers": [
            { "type": "tilelayer", "name": "sketch", "width": 3, "height": 3,
              "data": [1, 1, 1, 1, 6, 1, 1, 2, 5] }
        ]
    }"#;

    #[test]
    fn parses_dimensions_and_maps_gids_to_celltypes() {
        let s = parse_sketch(FIXTURE).unwrap();
        assert_eq!(s.width, 3);
        assert_eq!(s.height, 3);
        assert_eq!(s.regions.len(), 9);
        assert_eq!(s.regions[0], CellType::Ocean); // gid 1
        assert_eq!(s.regions[4], CellType::Rasconne); // gid 6
        assert_eq!(s.regions[7], CellType::Land); // gid 2
        assert_eq!(s.regions[8], CellType::Mountain); // gid 5
    }

    #[test]
    fn flip_flag_high_bits_are_ignored() {
        let flipped = 6u32 | 0x8000_0000;
        let json = format!(
            r#"{{"width":1,"height":1,"layers":[{{"type":"tilelayer","width":1,"height":1,"data":[{flipped}]}}]}}"#
        );
        let s = parse_sketch(&json).unwrap();
        assert_eq!(s.regions[0], CellType::Rasconne);
    }

    #[test]
    fn errors_when_there_is_no_tile_layer() {
        let json = r#"{"width":2,"height":2,"layers":[]}"#;
        assert!(parse_sketch(json).is_err());
    }
}
