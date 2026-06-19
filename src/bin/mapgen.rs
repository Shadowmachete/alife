//! Generate the static world from a sketch and save it as JSON.
//! Usage: `cargo run --bin mapgen [sketch.json]` (default: assets/sketch.json).
//! Writes out/alchaea.json. View it with `cargo run --bin mapview`.

use alife::terrain::save_json;
use alife::worldgen::{generate, ALCHAEA_SEED};
use std::path::Path;

fn main() {
    let sketch_path = std::env::args().nth(1).unwrap_or_else(|| "assets/sketch.json".into());
    let (w, h) = (120u32, 60u32);

    let json = std::fs::read_to_string(&sketch_path)
        .unwrap_or_else(|e| panic!("read sketch {sketch_path}: {e}"));
    let sketch = alife::sketch::parse_sketch(&json).expect("parse sketch");
    let map = generate(&sketch, w, h, ALCHAEA_SEED);

    std::fs::create_dir_all("out").expect("make out/");
    save_json(&map, Path::new("out/alchaea.json")).expect("save json");

    println!("generated {w}x{h} map (seed {:#x}) -> out/alchaea.json", map.seed());
}
