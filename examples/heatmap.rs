//! A quick ASCII visualisation of the valaar field — there is no graphical
//! viewer yet (rendering is a later plan), so this is the "for now" visual.
//! Run with: `cargo run --example heatmap`

use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::world::{Params, World};

fn main() {
    let (w, h) = (60u32, 30u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());

    // Rasconne reservoir: central surface source, linked to the deep reservoir.
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));
    world.add_access_point(w / 2, h / 2);

    let steps = 400;
    for _ in 0..steps {
        world.step();
    }

    render(&world, w, h, Layer::Surface, "surface");
    println!();
    render(&world, w, h, Layer::Underground, "underground");
}

fn render(world: &World<Grid2p5D>, w: u32, h: u32, layer: Layer, label: &str) {
    // Ten-step intensity ramp from empty to full.
    const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

    let val = |x: u32, y: u32| world.valaar.get(world.space.index(Coord::new(x, y, layer)));

    let mut max = 0.0f32;
    for y in 0..h {
        for x in 0..w {
            let v = val(x, y);
            if v > max {
                max = v;
            }
        }
    }

    println!("{label} valaar (max={max:.3}):");
    for y in 0..h {
        let mut line = String::with_capacity(w as usize);
        for x in 0..w {
            // sqrt spreads the low-end so the faint Dusk is still visible.
            let t = if max > 0.0 { (val(x, y) / max).sqrt() } else { 0.0 };
            let mut idx = (t * (RAMP.len() as f32 - 1.0)).round() as usize;
            if idx >= RAMP.len() {
                idx = RAMP.len() - 1;
            }
            line.push(RAMP[idx]);
        }
        println!("{line}");
    }
}
