# WASM mapview demo on GitHub Pages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the `mapview` egui viewer as a WASM demo served on GitHub Pages at `https://shadowmachete.github.io/alife/`, auto-rebuilt by a GitHub Actions workflow on every push to `main`, while keeping the native `cargo run --bin mapview` working unchanged.

**Architecture:** The viewer is already an `eframe::App`, so the port is small. `main` splits by target: native keeps `eframe::run_native` + CLI arg; a `#[cfg(target_arch="wasm32")]` `main` mounts `eframe::WebRunner` onto an HTML canvas. The native-only filesystem asset load (`std::fs::read`) is replaced on wasm by compile-time `include_str!`/`include_bytes!` of the default map + atlas, via a shared `build_tile_scene(xml, atlas_bytes)` helper that both targets call. Trunk bundles the wasm + an `index.html` into `dist/`; a Pages workflow builds with `trunk build --release --public-url /alife/` and deploys.

**Tech Stack:** Rust 2021, `eframe 0.29.1` (already a dep), Trunk 0.21 (build tool, not a crate dep). New **wasm-only** crate deps: `wasm-bindgen-futures`, `web-sys`. CI: `actions/deploy-pages`.

**Project root:** `~/dev/alife/`. All paths relative to it. Repo: `github.com/Shadowmachete/alife` (Pages serves under `/alife/`).

**Design source:** brainstorm of 2026-06-20 (this session). Toolchain verified present: `wasm32-unknown-unknown` target + `trunk 0.21.14`. eframe web API verified against the local `eframe-0.29.1` source (see Task 2).

**Builds on (shipped — exact interfaces consumed):**

- `src/bin/mapview.rs` — `MapApp::new(path: &str) -> Self` (calls `load_scene`); `load_scene(path: &str) -> Scene` (currently native-only, reads files); `Scene::Tiles(Box<TileSim>)`; the `.tmx` branch builds `TileSim { map, atlas, sim, mats, continents, n_continents, running }` then calls `t.reseed()`. Constants `SIM_SCALE`, `SEED_COUNT`, `WARM_STEPS`. `main() -> eframe::Result` calls `eframe::run_native("alife — map viewer", NativeOptions::default(), Box::new(move |_cc| Ok(Box::new(MapApp::new(&path)))))`.
- `eframe 0.29.1` web API (from `~/.cargo/registry/.../eframe-0.29.1`): `eframe::WebRunner::new()`; `runner.start(canvas: web_sys::HtmlCanvasElement, web_options: eframe::WebOptions, app_creator) -> impl Future<Output = Result<(), wasm_bindgen::JsValue>>`; `app_creator` is `Box<dyn FnOnce(&CreationContext) -> Result<Box<dyn App>, _>>` (same closure shape as the native `run_native` call already used); `eframe::wasm_bindgen` is re-exported (for the `JsCast` trait); `eframe::WebOptions::default()`.
- Assets on disk: `assets/alife_map_blended.tmx` (652 KB), `assets/sheet.rgba` (292 KB) — relative to `src/bin/mapview.rs` that is `../../assets/...`.
- `tilemap::{parse_tmx, material_grid, Atlas}`, `mapsim::{downscale, label_continents, world_from_materials, seed_on_fed_land}`, `sim::Sim`, `params::EcoParams` — all already imported by `mapview.rs` and all std-only (wasm-safe).

## Global Constraints

- **Language:** Rust, edition 2021. New deps are **wasm-target-only** (under `[target.'cfg(target_arch = "wasm32")'.dependencies]`) so native builds and the engine stay dependency-clean.
- **Native unchanged:** `cargo run --bin mapview [map]` and `cargo run --bin mapview out/x.json` must behave exactly as today. The engine + lib are untouched.
- **Formatting / lint gate (repo convention):** repo is **not** rustfmt-clean — **do NOT run `cargo fmt`**. Hand-format. Gate on `cargo test` + `cargo clippy --all-targets` (native target).
- **UI/build, not unit-tested:** like the egui rewrite, the viewer + build config are verified by **builds + clippy + a manual/CI run**, not unit tests. The "test to watch fail" for each task is the relevant build command failing before the change and passing after.
- **eframe API drift:** code below is written against **eframe 0.29.1** (verified in-cache). If the resolved version differs, re-check `WebRunner::start` and `WebOptions` on docs.rs and adjust the wasm `main` only.
- **Determinism / engine:** no engine logic changes; the sim stays deterministic.
- **Version control:** work on `main` (per user). Commit per task, plain message, **no `Co-Authored-By` trailer**. `git add` only the files each task names. Pushing + enabling Pages happens in Task 6 (the user approved going live).

---

## File Structure

- `src/bin/mapview.rs` — **modified**: extract `build_tile_scene`; cfg-split `load_scene` and `main`; add wasm web entry.
- `Cargo.toml` — **modified**: wasm-only deps; fix stale `license = "Unlicense"` → `"MIT"`.
- `index.html` — **new**: Trunk entry (canvas + `data-trunk` rust link).
- `Trunk.toml` — **new**: build target.
- `.github/workflows/pages.yml` — **new**: build with trunk + deploy to Pages.
- `.gitignore` — **modified**: ignore `dist/` and `target/` (if not already).
- `README.md` — **modified**: link the live demo + a "build the web demo" note.

---

### Task 1: Extract `build_tile_scene` (pure refactor, native still works)

**Files:**
- Modify: `src/bin/mapview.rs`

**Interfaces:**
- Produces: `build_tile_scene(xml: &str, atlas_bytes: &[u8]) -> Scene` — parses a `.tmx` string + `.rgba` atlas bytes into a live `Scene::Tiles`, identical to today's `.tmx` branch body. Consumed by `load_scene` (Task 3) on both targets.

- [ ] **Step 1: Read the current `.tmx` branch**

Run: `cd ~/dev/alife && sed -n '56,94p' src/bin/mapview.rs`
Expected: the current `load_scene` with the `if path.ends_with(".tmx")` block reading `xml` and atlas `bytes` from disk, then building `TileSim` and returning `Scene::Tiles(Box::new(t))`.

- [ ] **Step 2: Add the `build_tile_scene` helper**

In `src/bin/mapview.rs`, add this free function immediately **above** `fn load_scene`:

```rust
/// Build a live `Scene::Tiles` from an in-memory `.tmx` string and `.rgba` atlas
/// bytes. Shared by the native (file-backed) and wasm (embedded) loaders so the
/// scene-construction logic lives in one place.
fn build_tile_scene(xml: &str, atlas_bytes: &[u8]) -> Scene {
    let map = parse_tmx(xml).unwrap_or_else(|e| panic!("parse tmx: {e}"));
    let atlas = Atlas::load(atlas_bytes).unwrap_or_else(|e| panic!("atlas: {e}"));

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
}
```

- [ ] **Step 3: Call it from the native `.tmx` branch**

In `src/bin/mapview.rs`, replace the body of the `if path.ends_with(".tmx") {` branch (everything from `let mats = material_grid(...)` through `Scene::Tiles(Box::new(t))`) so the branch keeps only the file reads and delegates the rest:

```rust
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
```

(The `parse_tmx(&xml)` here is only needed to learn `map.atlas_image` for the atlas filename; `build_tile_scene` re-parses from the same string. Re-parsing once at load is negligible and keeps the helper self-contained.)

- [ ] **Step 4: Build + lint + run (native unchanged)**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: builds clean, clippy clean.

Manual check: `cargo run --release --bin mapview` still opens the textured live sim exactly as before.

- [ ] **Step 5: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "refactor: extract build_tile_scene from mapview load_scene"
```

---

### Task 2: WASM web entry + deps

**Files:**
- Modify: `Cargo.toml` (wasm-only deps + license fix)
- Modify: `src/bin/mapview.rs` (cfg-split `main`, add wasm `main`)

**Interfaces:**
- Consumes: `MapApp::new(&str)` (existing), `eframe::WebRunner` (verified API above).
- Produces: a wasm `main` that starts the app on canvas id `the_canvas_id`.

- [ ] **Step 1: Add wasm-only deps and fix the license in `Cargo.toml`**

In `Cargo.toml`, change the package license line:

```toml
license = "MIT"
```

And append, after the `[dependencies]` block:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = ["Document", "Element", "HtmlCanvasElement", "Window"] }
```

- [ ] **Step 2: Gate the existing native `main`**

In `src/bin/mapview.rs`, add the attribute directly above `fn main() -> eframe::Result {`:

```rust
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
```

(Leave the body unchanged.)

- [ ] **Step 3: Add the wasm `main`**

In `src/bin/mapview.rs`, add at the end of the file:

```rust
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
```

- [ ] **Step 4: Verify it FAILS to build for wasm (asset load still native-only)**

Run: `cd ~/dev/alife && cargo build --target wasm32-unknown-unknown --bin mapview 2>&1 | tail -20`
Expected: FAIL — `load_scene` still calls `std::fs` and `std::env`, which compile but `MapApp::new("")` → `load_scene("")` hits the `else` (json) branch calling `load_json`/filesystem; more importantly this confirms the wasm target compiles the entry but the asset path is wrong. (If it builds, that's fine too — Task 3 makes the wasm loader use embedded assets regardless.)

> This is the gate: the wasm `main` and deps now compile; Task 3 fixes the asset source.

- [ ] **Step 5: Verify native still builds + lints**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: native builds clean, clippy clean (clippy runs on the native target, so the wasm `main` is cfg'd out — that's expected).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/bin/mapview.rs
git commit -m "feat: wasm web entry for mapview (eframe WebRunner) + MIT license"
```

---

### Task 3: Embed assets on wasm (`load_scene` cfg-split)

**Files:**
- Modify: `src/bin/mapview.rs` (`load_scene` split by target)

**Interfaces:**
- Consumes: `build_tile_scene` (Task 1).
- Produces: a `#[cfg(target_arch="wasm32")] load_scene` that ignores its path and builds from embedded assets.

- [ ] **Step 1: Gate the existing native `load_scene`**

In `src/bin/mapview.rs`, add the attribute directly above `fn load_scene(path: &str) -> Scene {`:

```rust
#[cfg(not(target_arch = "wasm32"))]
fn load_scene(path: &str) -> Scene {
```

(Leave the body — now delegating to `build_tile_scene` from Task 1 — unchanged.)

- [ ] **Step 2: Add the wasm `load_scene` using embedded assets**

In `src/bin/mapview.rs`, immediately **after** the native `load_scene`, add:

```rust
/// Web build: assets are baked into the binary at compile time (no filesystem in
/// the browser). The path argument is ignored — the demo always shows the
/// default textured map.
#[cfg(target_arch = "wasm32")]
fn load_scene(_path: &str) -> Scene {
    let xml = include_str!("../../assets/alife_map_blended.tmx");
    let atlas_bytes = include_bytes!("../../assets/sheet.rgba");
    build_tile_scene(xml, atlas_bytes)
}
```

- [ ] **Step 3: Verify the wasm build is now clean**

Run: `cd ~/dev/alife && cargo build --target wasm32-unknown-unknown --bin mapview 2>&1 | tail -20`
Expected: PASS — `Finished` with no errors. (Warnings about unused `Path`/`std::fs` imports on wasm are possible; if any appear, scope the offending `use` lines with `#[cfg(not(target_arch = "wasm32"))]` and rebuild until clean.)

- [ ] **Step 4: Verify native still builds + lints + runs**

Run: `cd ~/dev/alife && cargo build --bin mapview && cargo clippy --all-targets`
Expected: clean.

Manual check: `cargo run --release --bin mapview` unchanged.

- [ ] **Step 5: Commit**

```bash
git add src/bin/mapview.rs
git commit -m "feat: embed map+atlas in the wasm mapview build"
```

---

### Task 4: Trunk config (`index.html` + `Trunk.toml`), local `dist/`

**Files:**
- Create: `index.html`
- Create: `Trunk.toml`
- Modify: `.gitignore` (ignore `dist/`)

**Interfaces:**
- Produces: a `trunk build` that emits `dist/` containing `index.html`, the wasm, and JS glue.

- [ ] **Step 1: Create `index.html`**

Create `index.html` at the repo root:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0, user-scalable=no" />
    <title>alife — map viewer</title>
    <style>
      html, body { margin: 0; padding: 0; height: 100%; width: 100%; overflow: hidden; background: #0b0d10; }
      canvas { width: 100%; height: 100%; display: block; }
    </style>
    <link data-trunk rel="rust" data-bin="mapview" data-wasm-opt="2" />
  </head>
  <body>
    <canvas id="the_canvas_id"></canvas>
  </body>
</html>
```

- [ ] **Step 2: Create `Trunk.toml`**

Create `Trunk.toml` at the repo root:

```toml
[build]
target = "index.html"

[serve]
open = false
```

- [ ] **Step 3: Ignore `dist/`**

Check `.gitignore`:

Run: `cd ~/dev/alife && cat .gitignore`

If `dist/` is not present, append it:

```
/dist
```

- [ ] **Step 4: Build the demo locally**

Run: `cd ~/dev/alife && trunk build --release --public-url /alife/ 2>&1 | tail -20`
Expected: `success`/`Finished`; `dist/` now exists.

Run: `ls dist/`
Expected: `index.html`, a `mapview-<hash>_bg.wasm`, a `mapview-<hash>.js`, and the embedded assets are *inside* the wasm (not separate files).

- [ ] **Step 5 (optional manual): serve locally**

Run: `cd ~/dev/alife && trunk serve --release` then open `http://localhost:8080`.
Expected: the textured live sim runs in the browser — drag pans, scroll zooms, the stats panel updates, Pause/Reseed work. (Stop with Ctrl-C.)

- [ ] **Step 6: Commit**

```bash
git add index.html Trunk.toml .gitignore
git commit -m "build: trunk config for the wasm mapview demo"
```

---

### Task 5: GitHub Actions Pages workflow

**Files:**
- Create: `.github/workflows/pages.yml`

**Interfaces:**
- Produces: a workflow that builds with trunk and deploys `dist/` to GitHub Pages on push to `main`.

- [ ] **Step 1: Create the workflow**

Create `.github/workflows/pages.yml`:

```yaml
name: Deploy mapview demo to Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

# Allow one concurrent deployment; cancel in-progress runs for the same ref.
concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: taiki-e/install-action@v2
        with:
          tool: trunk
      - name: Build (public path = /alife/)
        run: trunk build --release --public-url /alife/
      - uses: actions/upload-pages-artifact@v3
        with:
          path: dist

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

- [ ] **Step 2: Validate YAML locally**

Run: `cd ~/dev/alife && python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/pages.yml')); print('yaml ok')"`
Expected: `yaml ok`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/pages.yml
git commit -m "ci: build + deploy the wasm mapview demo to GitHub Pages"
```

---

### Task 6: Go live — push, enable Pages, verify

**Files:** none (deploy actions).

> This is the outward-facing publish step the user approved. Everything above must be green first.

- [ ] **Step 1: Final local gate**

Run: `cd ~/dev/alife && cargo test && cargo clippy --all-targets && cargo build --target wasm32-unknown-unknown --bin mapview && trunk build --release --public-url /alife/`
Expected: all PASS; `dist/` rebuilt.

- [ ] **Step 2: Push to origin/main**

```bash
git push origin main
```

- [ ] **Step 3: Enable Pages with the GitHub Actions source**

Try via the API (idempotent — handle "already exists"):

```bash
gh api -X POST repos/Shadowmachete/alife/pages -f build_type=workflow 2>/dev/null \
  || gh api -X PUT repos/Shadowmachete/alife/pages -f build_type=workflow
```

Expected: JSON describing the Pages site, or a 409 that the fallback `PUT` resolves. **If both fail with a permissions error,** report it and ask the user to set *Settings → Pages → Build and deployment → Source: GitHub Actions* once, then re-run the workflow with `gh workflow run "Deploy mapview demo to Pages"`.

- [ ] **Step 4: Watch the deploy**

```bash
gh run watch "$(gh run list --workflow=pages.yml --limit 1 --json databaseId --jq '.[0].databaseId')" --exit-status
```
Expected: the run succeeds (build + deploy jobs green).

- [ ] **Step 5: Verify the live site**

Run: `curl -sSI https://shadowmachete.github.io/alife/ | head -1`
Expected: `HTTP/2 200`. (First publish can take a minute; retry if 404.)

Manual check: open `https://shadowmachete.github.io/alife/` — the textured live sim runs.

---

### Task 7: Document the demo

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a demo link + web-build note**

In `README.md`, near the top (after the intro paragraph) add:

```markdown
**Live demo:** [shadowmachete.github.io/alife](https://shadowmachete.github.io/alife/) —
the `mapview` viewer compiled to WASM (drag to pan, scroll to zoom).
```

And in the "Running it" section, after the `mapview` block, add:

````markdown
Build the web demo locally (needs `trunk` + the `wasm32-unknown-unknown` target):

```sh
trunk serve --release            # http://localhost:8080
trunk build --release --public-url /alife/   # -> dist/ (what CI deploys)
```
````

- [ ] **Step 2: Lint + commit**

Run: `cd ~/dev/alife && cargo clippy --all-targets`
Expected: clippy clean.

```bash
git add README.md
git commit -m "docs: link the live WASM demo + web build steps"
git push origin main
```

---

## Self-Review

**Spec coverage:**
- WASM build of `mapview` → Tasks 2 (web entry), 3 (embedded assets). ✓
- GitHub Pages via Actions, auto-rebuild on push, `/alife/` subpath → Task 5 + Task 6 (`--public-url /alife/`). ✓
- Embed assets via `include_bytes!`/`include_str!` → Task 3. ✓
- Native viewer unchanged → cfg-gating in Tasks 2/3; native build+run checks each task. ✓
- Push + go live → Task 6 (push, enable Pages, verify 200). ✓
- License fix (Unlicense→MIT) → Task 2 Step 1. ✓

**Placeholder scan:** every code/config step shows full content; build steps give exact commands + expected output; eframe API is pinned to 0.29.1 with a drift note. No TBD/TODO. ✓

**Type/identifier consistency:** `build_tile_scene(xml: &str, atlas_bytes: &[u8]) -> Scene` defined in Task 1, consumed by both `load_scene` variants (Task 1 native, Task 3 wasm); canvas id `the_canvas_id` matches between the wasm `main` (Task 2) and `index.html` (Task 4); `data-bin="mapview"` matches the binary; `--public-url /alife/` consistent across Tasks 4/5/6; repo slug `Shadowmachete/alife` consistent in Task 6 + README. ✓

**Risks / mitigations:** (a) eframe web-API drift — pinned to verified 0.29.1, drift note included; (b) wasm build failing in CI first — Tasks 3/6 build for `wasm32` locally before any push; (c) Pages-enable permissions — Task 6 Step 3 has an explicit manual fallback; (d) possible unused-import warnings on the wasm target — Task 3 Step 3 says to cfg-scope them.

**Deferred / out of scope:** no engine or sim changes; the `.json` `TerrainMap` scene is native-only (not embedded); no service-worker/offline caching; no custom domain. The land-bridges/mutation-field plan (`2026-06-20-land-bridges-and-mutation-field.md`) is independent — if executed later it re-touches `load_scene`/`build_tile_scene`, a trivial merge.
