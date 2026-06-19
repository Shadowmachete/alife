//! Interactive map viewer. Left-drag = pan, scroll = zoom, Tab = toggle layer,
//! Esc = quit. Usage: `cargo run --bin mapview [map.json]`.
//! Loads the map (default out/alchaea.json); if absent, generates one from
//! assets/sketch.json with the canonical seed.

use alife::space::{Grid2p5D, Layer};
use alife::terrain::{load_json, TerrainMap};
use alife::viewer::{render_to_buffer, Camera};
use alife::worldgen::{generate, ALCHAEA_SEED};
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use std::path::Path;

const INIT_W: usize = 960;
const INIT_H: usize = 600;

fn load_or_generate(path: &str) -> TerrainMap {
    if Path::new(path).exists() {
        if let Ok(map) = load_json(Path::new(path)) {
            return map;
        }
    }
    let json = std::fs::read_to_string("assets/sketch.json").expect("read assets/sketch.json");
    let sketch = alife::sketch::parse_sketch(&json).expect("parse sketch");
    generate(&sketch, 120, 60, ALCHAEA_SEED)
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "out/alchaea.json".into());
    let map = load_or_generate(&path);
    let space = Grid2p5D::new(map.width(), map.height());

    let mut window = Window::new(
        "alife — map viewer  (drag: pan, scroll: zoom, Tab: layer, Esc: quit)",
        INIT_W,
        INIT_H,
        WindowOptions { resize: true, scale_mode: ScaleMode::Stretch, ..WindowOptions::default() },
    )
    .expect("open window");
    window.set_target_fps(60);

    let mut cam = Camera::new();
    let mut layer = Layer::Surface;
    let mut buf: Vec<u32> = vec![0; INIT_W * INIT_H];
    let mut last_drag: Option<(f32, f32)> = None;
    let mut tab_was_down = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let (vw, vh) = window.get_size();
        if buf.len() != vw * vh {
            buf = vec![0; vw * vh];
        }

        let mouse = window.get_mouse_pos(MouseMode::Pass);

        // Left-drag to pan.
        if window.get_mouse_down(MouseButton::Left) {
            if let (Some((lx, ly)), Some((mx, my))) = (last_drag, mouse) {
                cam.pan_pixels(mx - lx, my - ly);
            }
            last_drag = mouse;
        } else {
            last_drag = None;
        }

        // Scroll to zoom about the cursor.
        if let Some((_, sy)) = window.get_scroll_wheel() {
            if sy.abs() > 0.0 {
                let (mx, my) = mouse.unwrap_or((vw as f32 / 2.0, vh as f32 / 2.0));
                cam.zoom_at(mx, my, if sy > 0.0 { 1.1 } else { 1.0 / 1.1 });
            }
        }

        // Tab toggles layer (edge-triggered).
        let tab_down = window.is_key_down(Key::Tab);
        if tab_down && !tab_was_down {
            layer = match layer {
                Layer::Surface => Layer::Underground,
                Layer::Underground => Layer::Surface,
            };
        }
        tab_was_down = tab_down;

        render_to_buffer(&map, &space, layer, &cam, vw as u32, vh as u32, &mut buf);
        window.update_with_buffer(&buf, vw, vh).expect("update window");
    }
}
