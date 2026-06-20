//! Load a finite, CSV-encoded Tiled `.tmx` tile map and its atlas, and draw it
//! textured under the shared [`Camera`]. This is the textured counterpart to
//! `viewer::render_to_buffer` (which paints solid `CellType` colours): here each
//! map cell shows the actual 16×16 art tile the map authored in Tiled.
//!
//! Only what this project authors is supported: orthogonal maps, CSV tile data,
//! one inline tileset backed by a pre-baked RGBA atlas (`tools/png_to_rgba.py`),
//! and Tiled's H/V/D flip bits in the top three GID bits. Tiles from any *other*
//! tileset (GID outside the inline tileset's range) draw as backdrop.

use crate::viewer::{Camera, BACKDROP};

const FLIP_H: u32 = 0x8000_0000;
const FLIP_V: u32 = 0x4000_0000;
const FLIP_D: u32 = 0x2000_0000;
const GID_MASK: u32 = 0x1FFF_FFFF;

/// A decoded atlas: ARGB pixels (`0xAARRGGBB`), row-major `w×h`.
pub struct Atlas {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u32>,
}

impl Atlas {
    /// Parse a raw RGBA blob baked by `tools/png_to_rgba.py`
    /// (header `w:u32_le, h:u32_le`, then `w*h` RGBA bytes).
    pub fn load(bytes: &[u8]) -> Result<Atlas, String> {
        if bytes.len() < 8 {
            return Err("atlas blob too short for header".into());
        }
        let w = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let h = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let need = 8 + (w as usize) * (h as usize) * 4;
        if bytes.len() < need {
            return Err(format!("atlas truncated: {} bytes, need {}", bytes.len(), need));
        }
        let px = bytes[8..need]
            .chunks_exact(4)
            .map(|c| (c[3] as u32) << 24 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32)
            .collect();
        Ok(Atlas { w, h, px })
    }

    fn at(&self, x: u32, y: u32) -> u32 {
        self.px[(y * self.w + x) as usize]
    }
}

/// A finite orthogonal tile map: `gids[y*width + x]`, kept raw (flip bits intact).
pub struct TileMap {
    pub width: u32,
    pub height: u32,
    pub tile_w: u32,
    pub tile_h: u32,
    pub columns: u32,
    /// Every tileset's `firstgid`, ascending. Tiled may attach several tilesets
    /// that all point at the same sheet (e.g. an inline copy plus the external
    /// `.tsx`); a GID is resolved against the largest `firstgid` ≤ it.
    pub firstgids: Vec<u32>,
    pub atlas_image: String,
    pub gids: Vec<u32>,
}

/// Value of `name="..."` inside a single tag's text.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    Some(&tag[i..j])
}

/// The opening tag `<name ...>` starting at `xml[from..]` (text up to the next `>`).
fn open_tag<'a>(xml: &'a str, name: &str, from: usize) -> Option<&'a str> {
    let s = from + xml[from..].find(name)?;
    let e = s + xml[s..].find('>')?;
    Some(&xml[s..e])
}

/// Parse a finite, CSV-encoded `.tmx`. The inline tileset is the `<tileset>` that
/// directly contains an `<image>` (its `firstgid`/`columns`/image source drive
/// atlas lookup); a second external tileset, if any, is ignored.
pub fn parse_tmx(xml: &str) -> Result<TileMap, String> {
    let map = open_tag(xml, "<map", 0).ok_or("no <map> tag")?;
    let tile_w = attr(map, "tilewidth").and_then(|s| s.parse().ok()).ok_or("no tilewidth")?;
    let tile_h = attr(map, "tileheight").and_then(|s| s.parse().ok()).ok_or("no tileheight")?;

    let img_pos = xml.find("<image").ok_or("no inline <image> tileset")?;
    let ts_pos = xml[..img_pos].rfind("<tileset").ok_or("no <tileset> before <image>")?;
    let tileset = open_tag(xml, "<tileset", ts_pos).ok_or("bad <tileset>")?;
    let columns = attr(tileset, "columns").and_then(|s| s.parse().ok()).ok_or("no columns")?;
    let image = open_tag(xml, "<image", img_pos).ok_or("bad <image>")?;
    let atlas_image = attr(image, "source").ok_or("no image source")?.to_string();

    // Collect the firstgid of every tileset (they all share this one atlas).
    let mut firstgids: Vec<u32> = Vec::new();
    let mut scan = 0;
    while let Some(rel) = xml[scan..].find("<tileset") {
        let pos = scan + rel;
        if let Some(fg) = open_tag(xml, "<tileset", pos)
            .and_then(|t| attr(t, "firstgid"))
            .and_then(|s| s.parse().ok())
        {
            firstgids.push(fg);
        }
        scan = pos + "<tileset".len();
    }
    firstgids.sort_unstable();
    if firstgids.is_empty() {
        return Err("no tileset firstgid".into());
    }

    let layer_pos = xml.find("<layer").ok_or("no <layer>")?;
    let layer = open_tag(xml, "<layer", layer_pos).ok_or("bad <layer>")?;
    let width = attr(layer, "width").and_then(|s| s.parse().ok()).ok_or("no layer width")?;
    let height = attr(layer, "height").and_then(|s| s.parse().ok()).ok_or("no layer height")?;

    let marker = "encoding=\"csv\">";
    let ds = xml.find(marker).ok_or("no csv <data>")? + marker.len();
    let de = ds + xml[ds..].find("</data>").ok_or("unterminated <data>")?;
    let gids: Vec<u32> = xml[ds..de]
        .split(',')
        .filter_map(|t| {
            let t = t.trim();
            (!t.is_empty()).then(|| t.parse().map_err(|_| ()))
        })
        .collect::<Result<_, _>>()
        .map_err(|_| "non-numeric GID in csv".to_string())?;

    let expected = (width as usize) * (height as usize);
    if gids.len() != expected {
        return Err(format!("csv has {} gids, expected {width}×{height} = {expected}", gids.len()));
    }
    Ok(TileMap { width, height, tile_w, tile_h, columns, firstgids, atlas_image, gids })
}

/// Colour (`0x00RRGGBB`) at fractional map-tile coord `(wx, wy)`, or `None` for
/// out-of-bounds, empty, foreign-tileset, or transparent pixels.
fn sample(map: &TileMap, atlas: &Atlas, wx: f32, wy: f32) -> Option<u32> {
    if wx < 0.0 || wy < 0.0 {
        return None;
    }
    let (tx, ty) = (wx as u32, wy as u32);
    if tx >= map.width || ty >= map.height {
        return None;
    }
    let raw = map.gids[(ty * map.width + tx) as usize];
    let id = raw & GID_MASK;
    // Resolve against the owning tileset (largest firstgid ≤ id); gid 0 -> None.
    let base = map.firstgids.iter().copied().filter(|&fg| fg <= id).max()?;
    let local = id - base;
    let (col, row) = (local % map.columns, local / map.columns);

    let mut sx = (((wx - tx as f32) * map.tile_w as f32) as u32).min(map.tile_w - 1);
    let mut sy = (((wy - ty as f32) * map.tile_h as f32) as u32).min(map.tile_h - 1);
    if raw & FLIP_D != 0 {
        std::mem::swap(&mut sx, &mut sy);
    }
    if raw & FLIP_H != 0 {
        sx = map.tile_w - 1 - sx;
    }
    if raw & FLIP_V != 0 {
        sy = map.tile_h - 1 - sy;
    }

    let (ax, ay) = (col * map.tile_w + sx, row * map.tile_h + sy);
    if ax >= atlas.w || ay >= atlas.h {
        return None; // GID points outside this atlas (e.g. another tileset)
    }
    let argb = atlas.at(ax, ay);
    if argb >> 24 < 128 {
        return None; // transparent
    }
    Some(argb & 0x00FF_FFFF)
}

/// Fill `buf` (`width*height`, row-major) with the textured view of `map` under
/// `cam`. Empty/out-of-bounds/transparent pixels become [`BACKDROP`].
pub fn render_tiles_to_buffer(
    map: &TileMap,
    atlas: &Atlas,
    cam: &Camera,
    width: u32,
    height: u32,
    buf: &mut [u32],
) {
    for py in 0..height {
        let wy = cam.cy + py as f32 / cam.zoom;
        for px in 0..width {
            let wx = cam.cx + px as f32 / cam.zoom;
            buf[(py * width + px) as usize] = sample(map, atlas, wx, wy).unwrap_or(BACKDROP);
        }
    }
}

/// Classify every map cell into a coarse material by sampling its tile's centre
/// pixel against the three known fill colours. Drives a simulation on the drawn
/// map: grass→`Land`, plus `Ocean` and `Valaar`. (Connection tiles classify by
/// whichever material dominates their centre.)
pub fn material_grid(map: &TileMap, atlas: &Atlas) -> Vec<crate::terrain::CellType> {
    use crate::terrain::CellType;
    const REF: [([i64; 3], CellType); 3] = [
        ([52, 74, 97], CellType::Ocean),
        ([63, 116, 77], CellType::Land),
        ([121, 42, 44], CellType::Valaar),
    ];
    let mut out = Vec::with_capacity((map.width * map.height) as usize);
    for ty in 0..map.height {
        for tx in 0..map.width {
            let cell = match sample(map, atlas, tx as f32 + 0.5, ty as f32 + 0.5) {
                Some(rgb) => {
                    let (r, g, b) = ((rgb >> 16 & 255) as i64, (rgb >> 8 & 255) as i64, (rgb & 255) as i64);
                    REF.iter()
                        .min_by_key(|(c, _)| (r - c[0]).pow(2) + (g - c[1]).pow(2) + (b - c[2]).pow(2))
                        .unwrap()
                        .1
                }
                None => CellType::Ocean, // transparent/empty
            };
            out.push(cell);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba_blob(w: u32, h: u32, px: &[(u8, u8, u8, u8)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        for &(r, g, bl, a) in px {
            b.extend_from_slice(&[r, g, bl, a]);
        }
        b
    }

    #[test]
    fn atlas_load_unpacks_argb_and_dimensions() {
        let blob = rgba_blob(2, 1, &[(0x11, 0x22, 0x33, 0xFF), (0x44, 0x55, 0x66, 0x00)]);
        let a = Atlas::load(&blob).unwrap();
        assert_eq!((a.w, a.h), (2, 1));
        assert_eq!(a.at(0, 0), 0xFF11_2233);
        assert_eq!(a.at(1, 0), 0x0044_5566);
    }

    #[test]
    fn parse_reads_dims_tileset_and_gids() {
        let xml = r#"<?xml version="1.0"?>
<map tilewidth="16" tileheight="16">
 <tileset firstgid="1" columns="2"><image source="x.png" width="32" height="16"/></tileset>
 <tileset firstgid="99" source="other.tsx"/>
 <layer width="2" height="2">
  <data encoding="csv">
1,2,
3,4
  </data>
 </layer>
</map>"#;
        let m = parse_tmx(xml).unwrap();
        assert_eq!((m.width, m.height), (2, 2));
        assert_eq!((m.tile_w, m.tile_h), (16, 16));
        assert_eq!(m.columns, 2);
        assert_eq!(m.firstgids, vec![1, 99]); // both tilesets collected
        assert_eq!(m.atlas_image, "x.png");
        assert_eq!(m.gids, vec![1, 2, 3, 4]);
    }

    // 2×2 atlas of 1×1 tiles, 2-col; a 2×2 map showing each tile once.
    fn four_tile_scene() -> (TileMap, Atlas) {
        let a = Atlas::load(&rgba_blob(
            2,
            2,
            &[
                (0x11, 0x11, 0x11, 0xFF),
                (0x22, 0x22, 0x22, 0xFF),
                (0x33, 0x33, 0x33, 0xFF),
                (0x44, 0x44, 0x44, 0xFF),
            ],
        ))
        .unwrap();
        let m = TileMap {
            width: 2,
            height: 2,
            tile_w: 1,
            tile_h: 1,
            columns: 2,
            firstgids: vec![1],
            atlas_image: "x".into(),
            gids: vec![1, 2, 3, 4],
        };
        (m, a)
    }

    #[test]
    fn renders_each_gid_from_its_atlas_cell() {
        let (m, a) = four_tile_scene();
        let mut cam = Camera::new();
        cam.zoom = 1.0; // one screen pixel per tile
        let mut buf = vec![0u32; 4];
        render_tiles_to_buffer(&m, &a, &cam, 2, 2, &mut buf);
        assert_eq!(buf, vec![0x11_1111, 0x22_2222, 0x33_3333, 0x44_4444]);
    }

    #[test]
    fn out_of_bounds_is_backdrop() {
        let (m, a) = four_tile_scene();
        let mut cam = Camera::new();
        cam.zoom = 1.0;
        cam.cx = -1.0; // top-left pixel is off-map
        let mut buf = vec![0u32; 4];
        render_tiles_to_buffer(&m, &a, &cam, 2, 2, &mut buf);
        assert_eq!(buf[0], BACKDROP);
    }

    #[test]
    fn horizontal_flip_bit_mirrors_within_the_tile() {
        // one 2×2 tile; columns=1, firstgid=1
        let a = Atlas::load(&rgba_blob(
            2,
            2,
            &[
                (0xA0, 0, 0, 0xFF), // (0,0)
                (0xB0, 0, 0, 0xFF), // (1,0)
                (0xC0, 0, 0, 0xFF), // (0,1)
                (0xD0, 0, 0, 0xFF), // (1,1)
            ],
        ))
        .unwrap();
        let m = TileMap {
            width: 1,
            height: 1,
            tile_w: 2,
            tile_h: 2,
            columns: 1,
            firstgids: vec![1],
            atlas_image: "x".into(),
            gids: vec![1 | FLIP_H],
        };
        let mut cam = Camera::new();
        cam.zoom = 2.0; // 2 screen px per tile
        let mut buf = vec![0u32; 4];
        render_tiles_to_buffer(&m, &a, &cam, 2, 2, &mut buf);
        // top row mirrored: left shows (1,0)=B0, right shows (0,0)=A0
        assert_eq!(buf[0], 0xB0_0000);
        assert_eq!(buf[1], 0xA0_0000);
    }

    #[test]
    fn gid_beyond_atlas_is_backdrop() {
        let (mut m, a) = four_tile_scene();
        m.gids[0] = 99; // resolves past this 2×2 atlas
        let mut cam = Camera::new();
        cam.zoom = 1.0;
        let mut buf = vec![0u32; 4];
        render_tiles_to_buffer(&m, &a, &cam, 2, 2, &mut buf);
        assert_eq!(buf[0], BACKDROP);
    }

    // Regression: a tile placed from a *second* tileset (higher firstgid) that
    // shares the atlas must resolve to the same cell, not draw as backdrop.
    #[test]
    fn second_tileset_gid_resolves_to_same_atlas() {
        let (mut m, a) = four_tile_scene();
        m.firstgids = vec![1, 100]; // a second tileset sharing the atlas
        m.gids[0] = 100; // its tile 0 == atlas cell (0,0)
        m.gids[3] = 103; // its tile 3 == atlas cell (1,1)
        let mut cam = Camera::new();
        cam.zoom = 1.0;
        let mut buf = vec![0u32; 4];
        render_tiles_to_buffer(&m, &a, &cam, 2, 2, &mut buf);
        assert_eq!(buf[0], 0x11_1111, "gid 100 should map like gid 1");
        assert_eq!(buf[3], 0x44_4444, "gid 103 should map like gid 4");
    }

    #[test]
    fn material_grid_classifies_by_nearest_fill_colour() {
        use crate::terrain::CellType;
        // tiles 1,2,3 ≈ ocean, grass, valaar reference colours
        let a = Atlas::load(&rgba_blob(
            3,
            1,
            &[(52, 74, 97, 0xFF), (63, 116, 77, 0xFF), (121, 42, 44, 0xFF)],
        ))
        .unwrap();
        let m = TileMap {
            width: 3,
            height: 1,
            tile_w: 1,
            tile_h: 1,
            columns: 3,
            firstgids: vec![1],
            atlas_image: "x".into(),
            gids: vec![1, 2, 3],
        };
        assert_eq!(
            material_grid(&m, &a),
            vec![CellType::Ocean, CellType::Land, CellType::Valaar]
        );
    }
}
