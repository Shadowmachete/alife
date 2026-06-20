//! Interactive map viewer + live simulation.
//!   drag = pan, scroll = zoom, Esc = quit.
//!   .tmx map: textured, with a running sim — Space = pause/resume, R = reseed.
//!   .json map: a `TerrainMap` in solid `CellType` colours — Tab = toggle layer.
//! Usage: `cargo run --release --bin mapview [map]` (default alife_map_blended.tmx).

use alife::mapsim::{dot_color, downscale, seed_on_fed_land, world_from_materials};
use alife::params::EcoParams;
use alife::space::{Grid2p5D, Layer};
use alife::terrain::{load_json, TerrainMap};
use alife::tilemap::{material_grid, parse_tmx, render_tiles_to_buffer, Atlas, TileMap};
use alife::sim::Sim;
use alife::viewer::{render_to_buffer, Camera};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use std::path::Path;

const INIT_W: usize = 960;
const INIT_H: usize = 600;
const SIM_SCALE: u32 = 3; // sim grid is the display grid downscaled by this
const SEED_COUNT: usize = 1500;
const WARM_STEPS: usize = 150; // spread valaar before seeding life

/// A live tile simulation: the textured map, its atlas, and a sim running on a
/// downscaled material grid.
struct TileSim {
    map: TileMap,
    atlas: Atlas,
    sim: Sim<Grid2p5D>,
    mats: Vec<alife::terrain::CellType>, // sim-resolution materials
    running: bool,
}

impl TileSim {
    fn reseed(&mut self) {
        self.sim.pop = alife::population::Population::new();
        seed_on_fed_land(&mut self.sim, &self.mats, SEED_COUNT, 1e-3);
    }
}

enum Scene {
    Tiles(TileSim),
    Terrain { map: TerrainMap, space: Grid2p5D, layer: Layer },
}

impl Scene {
    fn dims(&self) -> (u32, u32) {
        match self {
            Scene::Tiles(t) => (t.map.width, t.map.height),
            Scene::Terrain { map, .. } => (map.width(), map.height()),
        }
    }
}

fn load_scene(path: &str) -> Scene {
    if path.ends_with(".tmx") {
        let xml = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let map = parse_tmx(&xml).unwrap_or_else(|e| panic!("parse {path}: {e}"));
        let dir = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
        let stem = Path::new(&map.atlas_image).file_stem().and_then(|s| s.to_str()).unwrap_or("sheet");
        let atlas_path = dir.join(format!("{stem}.rgba"));
        let bytes = std::fs::read(&atlas_path).unwrap_or_else(|e| {
            panic!("read atlas {}: {e}\nBake it first: `python3 tools/png_to_rgba.py`", atlas_path.display())
        });
        let atlas = Atlas::load(&bytes).unwrap_or_else(|e| panic!("atlas {}: {e}", atlas_path.display()));

        // Bridge the drawn map into a runnable sim (valaar from the Valaar cells).
        let mats = material_grid(&map, &atlas);
        let (sw, sh, sim_mats) = downscale(&mats, map.width, map.height, SIM_SCALE);
        let world = world_from_materials(sw, sh, &sim_mats);
        let mut sim = Sim::new(world, EcoParams::default(), 0xA11FE);
        for _ in 0..WARM_STEPS {
            sim.world.step();
        }
        let mut t = TileSim { map, atlas, sim, mats: sim_mats, running: true };
        t.reseed();
        Scene::Tiles(t)
    } else {
        let map = load_json(Path::new(path)).unwrap_or_else(|e| {
            panic!("load map {path}: {e}\nBuild one first, e.g. `python3 tools/sketch_to_map.py <sketch.png>`.")
        });
        let space = Grid2p5D::new(map.width(), map.height());
        Scene::Terrain { map, space, layer: Layer::Surface }
    }
}

/// Fit the whole map in the viewport and centre it.
fn framed_camera(mw: u32, mh: u32, vw: usize, vh: usize) -> Camera {
    let zoom = (vw as f32 / mw as f32).min(vh as f32 / mh as f32).max(0.25);
    Camera {
        cx: mw as f32 / 2.0 - (vw as f32 / zoom) / 2.0,
        cy: mh as f32 / 2.0 - (vh as f32 / zoom) / 2.0,
        zoom,
    }
}

/// Overlay each organism as a small filled square at its (scaled) display cell.
fn draw_dots(buf: &mut [u32], vw: u32, vh: u32, cam: &Camera, t: &TileSim, eco: &EcoParams) {
    let d = (cam.zoom * SIM_SCALE as f32 * 0.5).clamp(1.5, 9.0) as i32;
    for o in t.sim.pop.organisms() {
        let wx = (o.pos.x * SIM_SCALE + SIM_SCALE / 2) as f32;
        let wy = (o.pos.y * SIM_SCALE + SIM_SCALE / 2) as f32;
        let sx = ((wx - cam.cx) * cam.zoom) as i32;
        let sy = ((wy - cam.cy) * cam.zoom) as i32;
        let color = dot_color(o, eco);
        for oy in -d / 2..=d / 2 {
            for ox in -d / 2..=d / 2 {
                let (px, py) = (sx + ox, sy + oy);
                if px >= 0 && py >= 0 && (px as u32) < vw && (py as u32) < vh {
                    buf[(py as u32 * vw + px as u32) as usize] = color;
                }
            }
        }
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "assets/alife_map_blended.tmx".into());
    let mut scene = load_scene(&path);
    let (mw, mh) = scene.dims();

    let mut window = Window::new(
        "alife — map viewer",
        INIT_W,
        INIT_H,
        WindowOptions { resize: true, scale_mode: ScaleMode::Stretch, ..WindowOptions::default() },
    )
    .expect("open window");
    window.set_target_fps(60);

    let eco = EcoParams::default();
    let mut cam = framed_camera(mw, mh, INIT_W, INIT_H);
    let mut buf: Vec<u32> = vec![0; INIT_W * INIT_H];
    let mut last_drag: Option<(f32, f32)> = None;
    let mut frame: u64 = 0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let (vw, vh) = window.get_size();
        if buf.len() != vw * vh {
            buf = vec![0; vw * vh];
        }
        let mouse = window.get_mouse_pos(MouseMode::Pass);

        if window.get_mouse_down(MouseButton::Left) {
            if let (Some((lx, ly)), Some((mx, my))) = (last_drag, mouse) {
                cam.pan_pixels(mx - lx, my - ly);
            }
            last_drag = mouse;
        } else {
            last_drag = None;
        }
        if let Some((_, sy)) = window.get_scroll_wheel() {
            if sy.abs() > 0.0 {
                let (mx, my) = mouse.unwrap_or((vw as f32 / 2.0, vh as f32 / 2.0));
                cam.zoom_at(mx, my, if sy > 0.0 { 1.1 } else { 1.0 / 1.1 });
            }
        }

        match &mut scene {
            Scene::Tiles(t) => {
                if window.is_key_pressed(Key::Space, KeyRepeat::No) {
                    t.running = !t.running;
                }
                if window.is_key_pressed(Key::R, KeyRepeat::No) {
                    t.reseed();
                }
                if t.running {
                    t.sim.step();
                }
                // Half-tile offset: the autotiled tiles are a dual grid, so draw
                // them shifted +½ cell to put connection seams between cells.
                let tile_cam = Camera { cx: cam.cx + 0.5, cy: cam.cy + 0.5, zoom: cam.zoom };
                render_tiles_to_buffer(&t.map, &t.atlas, &tile_cam, vw as u32, vh as u32, &mut buf);
                draw_dots(&mut buf, vw as u32, vh as u32, &cam, t, &eco);
                if frame % 30 == 0 {
                    window.set_title(&format!(
                        "alife — pop {} | {:?} | year {} craw {} {}",
                        t.sim.population(),
                        t.sim.season(),
                        t.sim.calendar.year(),
                        t.sim.calendar.craw(),
                        if t.running { "(running, Space=pause R=reseed)" } else { "(paused)" },
                    ));
                }
            }
            Scene::Terrain { map, space, layer } => {
                if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
                    *layer = match layer {
                        Layer::Surface => Layer::Underground,
                        Layer::Underground => Layer::Surface,
                    };
                }
                render_to_buffer(map, space, *layer, &cam, vw as u32, vh as u32, &mut buf);
            }
        }

        window.update_with_buffer(&buf, vw, vh).expect("update window");
        frame += 1;
    }
}
