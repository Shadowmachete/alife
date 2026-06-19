//! Interactive map viewer *core*: a pan/zoom `Camera` and a pure framebuffer
//! renderer that paints each cell as a solid palette colour
//! (`CellType::fallback_rgb`). No windowing here — std-only and unit-tested. The
//! window/input shell is `bin/mapview.rs` (minifb), which calls
//! `render_to_buffer` each frame. This is the renderer the project keeps and
//! later grows to draw the live simulation.

use crate::space::{Coord, Layer, Space};
use crate::terrain::TerrainMap;

/// Base pixels-per-cell at zoom 1.0 (a 16×16 block per cell). [A9]
pub const CELL_PX: f32 = 16.0;

/// Colour (packed `0x00RRGGBB`) shown outside the map bounds.
pub const BACKDROP: u32 = 0x0010_1014;

/// A pan/zoom view. `(cx, cy)` is the map-cell coordinate (fractional) at the
/// viewport's top-left corner; `zoom` is pixels-per-cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub cx: f32,
    pub cy: f32,
    pub zoom: f32,
}

impl Camera {
    /// Start at the origin, one cell = `CELL_PX` pixels.
    pub fn new() -> Self {
        Camera { cx: 0.0, cy: 0.0, zoom: CELL_PX }
    }

    /// The map-cell coordinate (fractional) under a viewport pixel.
    pub fn screen_to_cell(&self, px: f32, py: f32) -> (f32, f32) {
        (self.cx + px / self.zoom, self.cy + py / self.zoom)
    }

    /// Pan by a screen-pixel delta (a mouse drag). Dragging right slides the view
    /// left across the map.
    pub fn pan_pixels(&mut self, dx: f32, dy: f32) {
        self.cx -= dx / self.zoom;
        self.cy -= dy / self.zoom;
    }

    /// Zoom by `factor` about a viewport pixel, keeping the map point under that
    /// pixel fixed. `zoom` is clamped to a sane range.
    pub fn zoom_at(&mut self, px: f32, py: f32, factor: f32) {
        let (wx, wy) = self.screen_to_cell(px, py);
        self.zoom = (self.zoom * factor).clamp(1.0, 256.0);
        self.cx = wx - px / self.zoom;
        self.cy = wy - py / self.zoom;
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera::new()
    }
}

/// Pack `[r, g, b]` into minifb's `0x00RRGGBB`.
fn pack(rgb: [u8; 3]) -> u32 {
    ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32
}

/// Fill `buf` (`width*height`, row-major) with the view of `map`'s `layer` under
/// `cam`. Each cell is a solid `fallback_rgb`; out-of-bounds cells are `BACKDROP`.
pub fn render_to_buffer<S: Space>(
    map: &TerrainMap,
    space: &S,
    layer: Layer,
    cam: &Camera,
    width: u32,
    height: u32,
    buf: &mut [u32],
) {
    let (mw, mh) = (map.width(), map.height());
    for py in 0..height {
        let wy = cam.cy + py as f32 / cam.zoom;
        for px in 0..width {
            let wx = cam.cx + px as f32 / cam.zoom;
            let color = if wx >= 0.0 && wy >= 0.0 && (wx as u32) < mw && (wy as u32) < mh {
                let c = Coord::new(wx as u32, wy as u32, layer);
                pack(map.get(space.index(c)).fallback_rgb())
            } else {
                BACKDROP
            };
            buf[(py * width + px) as usize] = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Coord, Grid2p5D, Layer, Space};
    use crate::terrain::{CellType, TerrainMap};

    fn pack(rgb: [u8; 3]) -> u32 {
        ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32
    }

    fn two_by_two() -> (Grid2p5D, TerrainMap) {
        let space = Grid2p5D::new(2, 2);
        let mut map = TerrainMap::filled(space.len(), 2, 2, CellType::Ocean, 0);
        map.set(space.index(Coord::new(1, 0, Layer::Surface)), CellType::Rasconne);
        (space, map)
    }

    #[test]
    fn renders_each_cell_in_its_fallback_colour() {
        let (space, map) = two_by_two();
        let cam = Camera::new(); // zoom 16, origin
        let (w, h) = (32u32, 32u32); // 2 cells * 16 px
        let mut buf = vec![0u32; (w * h) as usize];
        render_to_buffer(&map, &space, Layer::Surface, &cam, w, h, &mut buf);
        let at = |x: u32, y: u32| buf[(y * w + x) as usize];
        assert_eq!(at(8, 8), pack(CellType::Ocean.fallback_rgb())); // cell (0,0)
        assert_eq!(at(24, 8), pack(CellType::Rasconne.fallback_rgb())); // cell (1,0)
    }

    #[test]
    fn out_of_bounds_is_backdrop() {
        let (space, map) = two_by_two();
        let mut cam = Camera::new();
        cam.cx = -1.0; // shift so the top-left pixel is off-map
        let (w, h) = (32u32, 32u32);
        let mut buf = vec![0u32; (w * h) as usize];
        render_to_buffer(&map, &space, Layer::Surface, &cam, w, h, &mut buf);
        assert_eq!(buf[0], BACKDROP);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_fixed() {
        let mut cam = Camera::new();
        let (px, py) = (100.0, 60.0);
        let before = cam.screen_to_cell(px, py);
        cam.zoom_at(px, py, 2.0);
        let after = cam.screen_to_cell(px, py);
        assert!((before.0 - after.0).abs() < 1e-3, "x drifted: {before:?} {after:?}");
        assert!((before.1 - after.1).abs() < 1e-3, "y drifted");
        assert!((cam.zoom - 32.0).abs() < 1e-3);
    }

    #[test]
    fn pan_moves_the_view_in_cells() {
        let mut cam = Camera::new(); // zoom 16
        cam.pan_pixels(16.0, 32.0); // drag one cell right, two down
        assert!((cam.cx + 1.0).abs() < 1e-3);
        assert!((cam.cy + 2.0).abs() < 1e-3);
    }
}
