//! Interactive map viewer + live simulation, on egui/eframe.
//!   drag = pan, scroll = zoom.
//!   .tmx map: textured, with a running sim (Pause/Reseed in the side panel).
//!   .json map: a `TerrainMap` in solid `CellType` colours (layer toggle button).
//! Usage: `cargo run --release --bin mapview [map]` (default alife_map_blended.tmx).

use alife::mapsim::{
    add_underground_reservoirs, compute_stats, downscale, label_continents, marker_color,
    place_underground_reservoirs, seed_on_fed_land, world_from_materials,
};
use alife::bridges::{find_bridge_sites, open_bridge_cells, BridgeConfig, Bridges};
use alife::params::EcoParams;
use alife::rng::Rng;
use alife::history::{ContinentPoint, History, Snapshot};
use alife::season::CRAWS_PER_YEAR;
use alife::sim::Sim;
use alife::valaar::ValaarPhase;
use alife::world::Params;
use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::terrain::{CellType, TerrainMap};
#[cfg(not(target_arch = "wasm32"))]
use alife::terrain::load_json;
use alife::tilemap::{material_grid, parse_tmx, render_tiles_to_buffer, Atlas, TileMap};
use alife::viewer::{overlay_alpha, phase_rgb, render_to_buffer, Camera, CRYSTAL_RGB};
use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

const SIM_SCALE: u32 = 3; // sim grid is the display grid downscaled by this
const SEED_COUNT: usize = 1500;
const WARM_STEPS: usize = 150; // spread valaar before seeding life
const SIM_SEED: u64 = 0xA11FE;
const BRIDGE_SEED: u64 = 0xB12D6E;
const HISTORY_CAP: usize = 4000;
const SAMPLE_EVERY: u32 = 5;
const CAVE_BACKDROP: u32 = 0x000A_0A12; // dark rock for the Underground view

/// The editable working copy behind the Parameters panel.
#[derive(Clone)]
struct Tunables {
    eco: EcoParams,
    diffuse_rate: f32,
    decay: f32,
    bridges: BridgeConfig,
}

impl Default for Tunables {
    fn default() -> Self {
        let p = Params::default();
        Tunables {
            eco: EcoParams::default(),
            diffuse_rate: p.diffuse_rate,
            decay: p.decay,
            bridges: BridgeConfig::default(),
        }
    }
}

fn slider_f32(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.add(egui::Slider::new(v, range).text(label));
}

fn slider_u32(ui: &mut egui::Ui, label: &str, v: &mut u32, range: std::ops::RangeInclusive<u32>) {
    ui.add(egui::Slider::new(v, range).text(label));
}

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
    tunables: Tunables,
    history: History,
    show_charts: bool,
    show_valaar: bool,
    show_total: bool,
    continent_visible: Vec<bool>,
    view_layer: Layer,
}

impl TileSim {
    fn reseed(&mut self) {
        self.sim.pop = alife::population::Population::new();
        seed_on_fed_land(&mut self.sim, &self.mats, SEED_COUNT, 1e-3);
    }

    /// Fresh A/B run: rebuild the world + bridges + sim from the current
    /// tunables on the same map, re-warm valaar, and re-seed life.
    fn rebuild(&mut self) {
        let sw = self.sim.world.space.width();
        let sh = self.sim.world.space.height();
        self.sim = build_sim(&self.mats, sw, sh, &self.continents, &self.tunables);
        self.reseed();
        self.history.clear();
    }
}

// On wasm only `Tiles` is constructed (the .json `TerrainMap` scene is native).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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

/// Build a warmed `Sim` (world + bridges) for a tile scene from the downscaled
/// materials and the current tunables. Shared by first load and Reload.
fn build_sim(
    mats: &[CellType],
    sw: u32,
    sh: u32,
    continents: &[Option<u32>],
    tun: &Tunables,
) -> Sim<Grid2p5D> {
    let mut world = world_from_materials(sw, sh, mats);
    world.params.diffuse_rate = tun.diffuse_rate;
    world.params.decay = tun.decay;
    let reservoirs = place_underground_reservoirs(sw, sh, continents);
    add_underground_reservoirs(&mut world, &reservoirs);
    let mut sim = Sim::new(world, tun.eco, SIM_SEED);
    let sites = find_bridge_sites(mats, sw, sh, continents, &mut Rng::new(BRIDGE_SEED), &tun.bridges);
    sim.set_bridges(Bridges::new(sites, tun.bridges, BRIDGE_SEED));
    for _ in 0..WARM_STEPS {
        sim.world.step();
    }
    sim
}

/// Build a live `Scene::Tiles` from an in-memory `.tmx` string and `.rgba` atlas
/// bytes. Shared by the native (file-backed) and wasm (embedded) loaders so the
/// scene-construction logic lives in one place.
fn build_tile_scene(xml: &str, atlas_bytes: &[u8]) -> Scene {
    let map = parse_tmx(xml).unwrap_or_else(|e| panic!("parse tmx: {e}"));
    let atlas = Atlas::load(atlas_bytes).unwrap_or_else(|e| panic!("atlas: {e}"));

    let mats = material_grid(&map, &atlas);
    let (sw, sh, sim_mats) = downscale(&mats, map.width, map.height, SIM_SCALE);
    let (continents, n_continents) = label_continents(&sim_mats, sw, sh);
    let tunables = Tunables::default();
    let sim = build_sim(&sim_mats, sw, sh, &continents, &tunables);
    let mut t = TileSim {
        map,
        atlas,
        sim,
        mats: sim_mats,
        continents,
        n_continents,
        running: true,
        tunables,
        history: History::new(HISTORY_CAP),
        show_charts: false,
        show_valaar: true,
        show_total: true,
        continent_visible: vec![true; n_continents as usize],
        view_layer: Layer::Surface,
    };
    t.reseed();
    Scene::Tiles(Box::new(t))
}

#[cfg(not(target_arch = "wasm32"))]
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
        build_tile_scene(&xml, &bytes)
    } else {
        let map = load_json(Path::new(path)).unwrap_or_else(|e| {
            panic!("load map {path}: {e}\nBuild one first, e.g. `python3 tools/sketch_to_map.py <sketch.png>`.")
        });
        let space = Grid2p5D::new(map.width(), map.height());
        Scene::Terrain { map, space, layer: Layer::Surface }
    }
}

/// Web build: assets are baked into the binary at compile time (no filesystem in
/// the browser). The path argument is ignored — the demo always shows the
/// default textured map.
#[cfg(target_arch = "wasm32")]
fn load_scene(_path: &str) -> Scene {
    let xml = include_str!("../../assets/alife_map_blended.tmx");
    let atlas_bytes = include_bytes!("../../assets/sheet.rgba");
    build_tile_scene(xml, atlas_bytes)
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
            if t.view_layer == Layer::Underground {
                buf.iter_mut().for_each(|p| *p = CAVE_BACKDROP);
            } else {
                // Half-tile offset: autotiled tiles are a dual grid; shift +½ cell.
                let tile_cam = Camera { cx: cam.cx + 0.5, cy: cam.cy + 0.5, zoom: cam.zoom };
                render_tiles_to_buffer(&t.map, &t.atlas, &tile_cam, vw, vh, buf);
            }
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

/// Paint each currently-open land bridge cell as a sandy ribbon. The static
/// tile layer can't reflect the dynamic passability flips, so this overlays the
/// risen land (under the organisms) wherever an ocean cell has become passable.
fn draw_bridges(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim) {
    let cell_px = cam.zoom * SIM_SCALE as f32;
    let sw = t.sim.world.space.width();
    let sand = egui::Color32::from_rgb(206, 175, 120);
    for i in open_bridge_cells(&t.mats, t.sim.world.passability()) {
        let cx = (i as u32 % sw) * SIM_SCALE;
        let cy = (i as u32 / sw) * SIM_SCALE;
        let sx = rect.min.x + (cx as f32 - cam.cx) * cam.zoom;
        let sy = rect.min.y + (cy as f32 - cam.cy) * cam.zoom;
        let cell = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(cell_px, cell_px));
        if rect.intersects(cell) {
            painter.rect_filled(cell, 0.0, sand);
        }
    }
}

/// Paint a translucent valaar overlay over the map for `layer`: per sim-cell, a
/// rectangle whose opacity tracks the cell's valaar (normalised by the layer's
/// peak) and whose hue is the current phase. Cells where frozen `crystal`
/// outweighs liquid valaar are drawn in the icy `CRYSTAL_RGB` instead.
fn draw_valaar(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim, layer: Layer) {
    let space = &t.sim.world.space;
    let (sw, sh) = (space.width(), space.height());
    let hue = phase_rgb(ValaarPhase::for_season(t.sim.season()));
    // Per-frame, per-layer peak for normalisation [V4].
    let mut max_v = 0.0f32;
    for y in 0..sh {
        for x in 0..sw {
            let i = space.index(Coord::new(x, y, layer));
            max_v = max_v.max(t.sim.world.valaar.get(i)).max(t.sim.world.crystal.get(i));
        }
    }
    let cell_px = cam.zoom * SIM_SCALE as f32;
    for y in 0..sh {
        for x in 0..sw {
            let i = space.index(Coord::new(x, y, layer));
            let v = t.sim.world.valaar.get(i);
            let cr = t.sim.world.crystal.get(i);
            let (rgb, amount) = if cr > v { (CRYSTAL_RGB, cr) } else { (hue, v) };
            let a = overlay_alpha(amount, max_v);
            if a == 0 {
                continue;
            }
            let dx = (x * SIM_SCALE) as f32;
            let dy = (y * SIM_SCALE) as f32;
            let sx = rect.min.x + (dx - cam.cx) * cam.zoom;
            let sy = rect.min.y + (dy - cam.cy) * cam.zoom;
            let cell = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(cell_px, cell_px));
            if rect.intersects(cell) {
                painter.rect_filled(
                    cell,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], a),
                );
            }
        }
    }
}

/// Draw each organism as an outlined circle inside `rect`: radius scales with
/// body size and zoom, fill by diet, with a darker ring behind for contrast.
fn draw_organisms(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, t: &TileSim, layer: Layer) {
    let cell_px = cam.zoom * SIM_SCALE as f32;
    for o in t.sim.pop.organisms() {
        if o.pos.layer != layer {
            continue;
        }
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

/// The collapsible Parameters panel: sliders for every dial. Ecology applies
/// live; valaar/bridge sliders are staged and take effect on Reload (handled by
/// the caller). Returns `true` if the Reload button was clicked.
fn parameters_ui(ui: &mut egui::Ui, tun: &mut Tunables) -> bool {
    let mut reload = false;
    egui::CollapsingHeader::new("Parameters").default_open(false).show(ui, |ui| {
        ui.label(egui::RichText::new("Ecology (live)").strong());
        let e = &mut tun.eco;
        slider_f32(ui, "uptake_rate", &mut e.uptake_rate, 0.0..=1.0);
        slider_f32(ui, "move_cost", &mut e.move_cost, 0.0..=0.2);
        slider_f32(ui, "basal_cost", &mut e.basal_cost, 0.0..=0.2);
        slider_f32(ui, "repro_cost_fraction", &mut e.repro_cost_fraction, 0.0..=1.0);
        slider_f32(ui, "mutation_rate", &mut e.mutation_rate, 0.0..=0.5);
        slider_f32(ui, "rasgun_mutation_mult", &mut e.rasgun_mutation_mult, 1.0..=5.0);
        slider_f32(ui, "predation_efficiency", &mut e.predation_efficiency, 0.0..=1.0);
        slider_f32(ui, "heat_stress", &mut e.heat_stress, 0.0..=1.0);
        slider_f32(ui, "drought_stress", &mut e.drought_stress, 0.0..=1.0);
        slider_f32(ui, "detritus_fraction", &mut e.detritus_fraction, 0.0..=1.0);
        slider_f32(ui, "valaar_drain", &mut e.valaar_drain, 0.0..=1.0);
        slider_f32(ui, "base_energy", &mut e.base_energy, 0.5..=16.0);
        slider_f32(ui, "size_energy", &mut e.size_energy, 0.0..=32.0);
        slider_f32(ui, "size_cost", &mut e.size_cost, 0.0..=0.2);
        slider_f32(ui, "initial_energy", &mut e.initial_energy, 0.5..=16.0);
        slider_u32(ui, "min_lifespan", &mut e.min_lifespan, 1..=1000);
        slider_u32(ui, "max_lifespan", &mut e.max_lifespan, 1..=4000);

        ui.separator();
        ui.label(egui::RichText::new("Valaar (reload)").strong());
        slider_f32(ui, "diffuse_rate", &mut tun.diffuse_rate, 0.0..=0.24);
        slider_f32(ui, "decay", &mut tun.decay, 0.0..=0.2);

        ui.separator();
        ui.label(egui::RichText::new("Bridges (reload)").strong());
        let b = &mut tun.bridges;
        slider_u32(ui, "max_gap", &mut b.max_gap, 1..=30);
        slider_u32(ui, "min_width", &mut b.min_width, 1..=10);
        slider_u32(ui, "max_width", &mut b.max_width, 1..=12);
        slider_f32(ui, "site_fraction", &mut b.site_fraction, 0.0..=1.0);
        slider_f32(ui, "open_fraction", &mut b.open_fraction, 0.0..=1.0);
        slider_u32(ui, "min_duration", &mut b.min_duration, 1..=117);
        slider_u32(ui, "max_duration", &mut b.max_duration, 1..=117);

        ui.separator();
        if ui.button("Reload (fresh run)").clicked() {
            reload = true;
        }
    });
    reload
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
        // Advance the live sim each frame (Tiles scene only, when running) and
        // sample the population into the history buffer for the charts.
        if let Scene::Tiles(t) = &mut self.scene {
            if t.running {
                t.sim.step();
                if t.sim.calendar.craw() % SAMPLE_EVERY == 0 {
                    let sw = t.sim.world.space.width();
                    let s = compute_stats(&t.sim.pop, sw, &t.continents, t.n_continents);
                    let x = (t.sim.calendar.year() * CRAWS_PER_YEAR + t.sim.calendar.craw()) as f64;
                    let continents = s
                        .continents
                        .iter()
                        .map(|cs| ContinentPoint { label: cs.label, count: cs.count, mean_size: cs.mean_size })
                        .collect();
                    t.history.push(Snapshot { x, total: s.total, mean_size: s.mean_size, continents });
                }
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
                    for cs in s.continents.iter().filter(|cs| cs.count > 0).take(8) {
                        ui.label(format!("  c{}: {} (size {:.2})", cs.label, cs.count, cs.mean_size));
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
                    ui.horizontal(|ui| {
                        let label = match t.view_layer {
                            Layer::Surface => "Flip to underground",
                            Layer::Underground => "Flip to surface",
                        };
                        if ui.button(label).clicked() {
                            t.view_layer = match t.view_layer {
                                Layer::Surface => Layer::Underground,
                                Layer::Underground => Layer::Surface,
                            };
                        }
                    });
                    ui.separator();
                    let reload = parameters_ui(ui, &mut t.tunables);
                    t.sim.eco = t.tunables.eco; // ecology applies live
                    if reload {
                        t.rebuild();
                    }
                    ui.separator();
                    ui.checkbox(&mut t.show_charts, "Charts");
                    ui.checkbox(&mut t.show_valaar, "Valaar");
                    if t.show_valaar {
                        let phase = ValaarPhase::for_season(t.sim.season());
                        let [r, g, b] = phase_rgb(phase);
                        ui.horizontal(|ui| {
                            let (resp, painter) =
                                ui.allocate_painter(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            painter.rect_filled(resp.rect, 2.0, egui::Color32::from_rgb(r, g, b));
                            ui.label(format!("phase: {phase:?}"));
                        });
                    }
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

        if let Scene::Tiles(t) = &mut self.scene {
            if t.show_charts {
                egui::TopBottomPanel::bottom("charts")
                    .resizable(true)
                    .default_height(240.0)
                    .show(ctx, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut t.show_total, "total / mean");
                            for (label, vis) in t.continent_visible.iter_mut().enumerate() {
                                ui.checkbox(vis, format!("c{label}"));
                            }
                        });

                        ui.label("Population");
                        Plot::new("population_plot")
                            .legend(Legend::default())
                            .height(90.0)
                            .show(ui, |pui| {
                                if t.show_total {
                                    pui.line(Line::new(PlotPoints::from(t.history.total_series())).name("total"));
                                }
                                for (label, vis) in t.continent_visible.iter().enumerate() {
                                    if *vis {
                                        let pts = t.history.continent_count_series(label as u32);
                                        if !pts.is_empty() {
                                            pui.line(Line::new(PlotPoints::from(pts)).name(format!("c{label}")));
                                        }
                                    }
                                }
                            });

                        ui.label("Mean body size");
                        Plot::new("size_plot")
                            .legend(Legend::default())
                            .height(90.0)
                            .show(ui, |pui| {
                                if t.show_total {
                                    pui.line(Line::new(PlotPoints::from(t.history.mean_size_series())).name("mean"));
                                }
                                for (label, vis) in t.continent_visible.iter().enumerate() {
                                    if *vis {
                                        let pts = t.history.continent_size_series(label as u32);
                                        if !pts.is_empty() {
                                            pui.line(Line::new(PlotPoints::from(pts)).name(format!("c{label}")));
                                        }
                                    }
                                }
                            });
                    });
            }
        }

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
                let painter = ui.painter_at(rect); // clip overlays to the map rect
                if t.view_layer == Layer::Surface {
                    draw_bridges(&painter, rect, &self.cam, t);
                }
                if t.show_valaar {
                    draw_valaar(&painter, rect, &self.cam, t, t.view_layer);
                }
                draw_organisms(&painter, rect, &self.cam, t, t.view_layer);
            }
        });

        ctx.request_repaint(); // keep animating the live sim
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let path = std::env::args().nth(1).unwrap_or_else(|| "assets/alife_map_blended.tmx".into());
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "alife — map viewer",
        native_options,
        Box::new(move |_cc| Ok(Box::new(MapApp::new(&path)))),
    )
}

/// Web entry: mount the eframe app onto the `<canvas id="the_canvas_id">` that
/// `index.html` provides. (Trunk calls `main` automatically.)
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("missing canvas #the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#the_canvas_id is not a <canvas>");
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(MapApp::new("")))), // path ignored on wasm
            )
            .await
            .expect("failed to start eframe");
    });
}
