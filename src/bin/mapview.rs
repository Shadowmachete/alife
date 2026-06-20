//! Interactive map viewer + live simulation, on egui/eframe.
//!   drag = pan, scroll = zoom.
//!   .tmx map: textured, with a running sim (Pause/Reseed in the side panel).
//!   .json map: a `TerrainMap` in solid `CellType` colours (layer toggle button).
//! Usage: `cargo run --release --bin mapview [map]` (default alife_map_blended.tmx).

use alife::mapsim::{
    compute_stats, downscale, label_continents, marker_color, seed_on_fed_land, world_from_materials,
};
use alife::params::EcoParams;
use alife::sim::Sim;
use alife::space::{Grid2p5D, Layer, Space};
use alife::terrain::{load_json, CellType, TerrainMap};
use alife::tilemap::{material_grid, parse_tmx, render_tiles_to_buffer, Atlas, TileMap};
use alife::viewer::{render_to_buffer, Camera};
use eframe::egui;
use std::path::Path;

const SIM_SCALE: u32 = 3; // sim grid is the display grid downscaled by this
const SEED_COUNT: usize = 1500;
const WARM_STEPS: usize = 150; // spread valaar before seeding life

/// A live tile simulation: the textured map, its atlas, a sim on a downscaled
/// material grid, and cached continent labels for the stats panel.
struct TileSim {
    map: TileMap,
    atlas: Atlas,
    sim: Sim<Grid2p5D>,
    mats: Vec<CellType>,          // sim-resolution materials
    continents: Vec<Option<u32>>, // per sim-cell continent label
    n_continents: u32,
    running: bool,
}

impl TileSim {
    fn reseed(&mut self) {
        self.sim.pop = alife::population::Population::new();
        seed_on_fed_land(&mut self.sim, &self.mats, SEED_COUNT, 1e-3);
    }
}

enum Scene {
    Tiles(Box<TileSim>),
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

        let mats = material_grid(&map, &atlas);
        let (sw, sh, sim_mats) = downscale(&mats, map.width, map.height, SIM_SCALE);
        let (continents, n_continents) = label_continents(&sim_mats, sw, sh);
        let world = world_from_materials(sw, sh, &sim_mats);
        let mut sim = Sim::new(world, EcoParams::default(), 0xA11FE);
        for _ in 0..WARM_STEPS {
            sim.world.step();
        }
        let mut t = TileSim {
            map,
            atlas,
            sim,
            mats: sim_mats,
            continents,
            n_continents,
            running: true,
        };
        t.reseed();
        Scene::Tiles(Box::new(t))
    } else {
        let map = load_json(Path::new(path)).unwrap_or_else(|e| {
            panic!("load map {path}: {e}\nBuild one first, e.g. `python3 tools/sketch_to_map.py <sketch.png>`.")
        });
        let space = Grid2p5D::new(map.width(), map.height());
        Scene::Terrain { map, space, layer: Layer::Surface }
    }
}

/// Fit the whole map in a `vw×vh` viewport and centre it.
fn framed_camera(mw: u32, mh: u32, vw: usize, vh: usize) -> Camera {
    let zoom = (vw as f32 / mw as f32).min(vh as f32 / mh as f32).max(0.25);
    Camera {
        cx: mw as f32 / 2.0 - (vw as f32 / zoom) / 2.0,
        cy: mh as f32 / 2.0 - (vh as f32 / zoom) / 2.0,
        zoom,
    }
}

/// CPU-render the current scene's terrain into `buf` (sized `vw*vh`) under `cam`.
fn render_terrain(scene: &Scene, cam: &Camera, vw: u32, vh: u32, buf: &mut [u32]) {
    match scene {
        Scene::Tiles(t) => {
            // Half-tile offset: autotiled tiles are a dual grid; shift +½ cell.
            let tile_cam = Camera { cx: cam.cx + 0.5, cy: cam.cy + 0.5, zoom: cam.zoom };
            render_tiles_to_buffer(&t.map, &t.atlas, &tile_cam, vw, vh, buf);
        }
        Scene::Terrain { map, space, layer } => {
            render_to_buffer(map, space, *layer, cam, vw, vh, buf);
        }
    }
}

/// Convert a `0x00RRGGBB` buffer into an egui image.
fn to_color_image(buf: &[u32], w: usize, h: usize) -> egui::ColorImage {
    let pixels = buf
        .iter()
        .map(|&p| egui::Color32::from_rgb((p >> 16) as u8, (p >> 8) as u8, p as u8))
        .collect();
    egui::ColorImage { size: [w, h], pixels }
}

/// Draw each organism as an outlined circle inside `rect`: radius scales with
/// body size and zoom, fill by diet, with a darker ring behind for contrast.
fn draw_organisms(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim) {
    let cell_px = cam.zoom * SIM_SCALE as f32;
    for o in t.sim.pop.organisms() {
        let wx = (o.pos.x * SIM_SCALE + SIM_SCALE / 2) as f32;
        let wy = (o.pos.y * SIM_SCALE + SIM_SCALE / 2) as f32;
        let sx = rect.min.x + (wx - cam.cx) * cam.zoom;
        let sy = rect.min.y + (wy - cam.cy) * cam.zoom;
        let center = egui::pos2(sx, sy);
        if !rect.contains(center) {
            continue;
        }
        let radius = ((0.25 + 0.5 * o.genome.size) * cell_px * 0.5).clamp(1.5, 64.0);
        let c = marker_color(o);
        let fill = egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        painter.circle_filled(center, radius + 1.0, egui::Color32::from_black_alpha(220)); // ring
        painter.circle_filled(center, radius, fill);
    }
}

struct MapApp {
    scene: Scene,
    cam: Camera,
    framed: bool,
    buf: Vec<u32>,
    tex: Option<egui::TextureHandle>,
}

impl MapApp {
    fn new(path: &str) -> Self {
        let scene = load_scene(path);
        MapApp { scene, cam: Camera::new(), framed: false, buf: Vec::new(), tex: None }
    }
}

impl eframe::App for MapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Advance the live sim each frame (Tiles scene only, when running).
        if let Scene::Tiles(t) = &mut self.scene {
            if t.running {
                t.sim.step();
            }
        }

        egui::SidePanel::right("stats").default_width(220.0).show(ctx, |ui| {
            ui.heading("alife");
            match &mut self.scene {
                Scene::Tiles(t) => {
                    let sw = t.sim.world.space.width();
                    let s = compute_stats(&t.sim.pop, sw, &t.continents, t.n_continents);
                    ui.label(format!("season {:?}", t.sim.season()));
                    ui.label(format!("year {}  craw {}", t.sim.calendar.year(), t.sim.calendar.craw()));
                    ui.separator();
                    ui.label(format!("population  {}", s.total));
                    ui.label(format!("autotrophs  {}", s.autotrophs));
                    ui.label(format!("predators   {}", s.predators));
                    ui.label(format!("mean size   {:.2}", s.mean_size));
                    ui.separator();
                    ui.label(format!("continents: {}", t.n_continents));
                    for (label, count) in s.continents.iter().filter(|(_, c)| *c > 0).take(8) {
                        ui.label(format!("  c{label}: {count}"));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(if t.running { "Pause" } else { "Resume" }).clicked() {
                            t.running = !t.running;
                        }
                        if ui.button("Reseed").clicked() {
                            t.reseed();
                        }
                    });
                }
                Scene::Terrain { layer, .. } => {
                    ui.label(format!("layer: {layer:?}"));
                    if ui.button("Toggle layer").clicked() {
                        *layer = match layer {
                            Layer::Surface => Layer::Underground,
                            Layer::Underground => Layer::Surface,
                        };
                    }
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let (vw, vh) = (avail.x.max(1.0) as u32, avail.y.max(1.0) as u32);

            // Frame the map the first time we know the viewport size.
            if !self.framed {
                let (mw, mh) = self.scene.dims();
                self.cam = framed_camera(mw, mh, vw as usize, vh as usize);
                self.framed = true;
            }

            // CPU-render terrain into our buffer and upload it as a texture.
            let len = (vw * vh) as usize;
            if self.buf.len() != len {
                self.buf = vec![0u32; len];
            }
            render_terrain(&self.scene, &self.cam, vw, vh, &mut self.buf);
            let img = to_color_image(&self.buf, vw as usize, vh as usize);
            match &mut self.tex {
                Some(handle) => handle.set(img, egui::TextureOptions::NEAREST),
                none => *none = Some(ui.ctx().load_texture("terrain", img, egui::TextureOptions::NEAREST)),
            }
            let tex_id = self.tex.as_ref().unwrap().id();

            // Draw the terrain to fill the panel and capture pan/zoom input.
            let (rect, response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
            ui.painter().image(
                tex_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            if response.dragged() {
                let d = response.drag_delta();
                self.cam.pan_pixels(d.x, d.y);
            }
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(pos) = response.hover_pos() {
                    let local = pos - rect.min;
                    self.cam.zoom_at(local.x, local.y, if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 });
                }
            }

            if let Scene::Tiles(t) = &self.scene {
                let painter = ui.painter_at(rect); // clip organisms to the map rect
                draw_organisms(&painter, rect, &self.cam, t);
            }
        });

        ctx.request_repaint(); // keep animating the live sim
    }
}

fn main() -> eframe::Result {
    let path = std::env::args().nth(1).unwrap_or_else(|| "assets/alife_map_blended.tmx".into());
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "alife — map viewer",
        native_options,
        Box::new(move |_cc| Ok(Box::new(MapApp::new(&path)))),
    )
}
