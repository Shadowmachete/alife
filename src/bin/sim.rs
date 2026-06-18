//! Headless substrate demo: grow a valaar gradient from a central Rasconne
//! source and print the centre-vs-edge values. No graphics yet.

use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::world::{Params, World};

fn main() {
    let (w, h) = (64u32, 64u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());

    // Rasconne reservoir: a central surface source, linked to the deep reservoir.
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));
    world.add_access_point(w / 2, h / 2);

    let steps = 500;
    for _ in 0..steps {
        world.step();
    }

    let at = |world: &World<Grid2p5D>, x, y, layer| {
        world.valaar.get(world.space.index(Coord::new(x, y, layer)))
    };
    println!(
        "after {steps} steps: total={:.3} center={:.3} edge={:.3} underground_center={:.3}",
        world.valaar.total(),
        at(&world, w / 2, h / 2, Layer::Surface),
        at(&world, 0, 0, Layer::Surface),
        at(&world, w / 2, h / 2, Layer::Underground),
    );
}
