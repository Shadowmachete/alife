use alife::space::{Coord, Grid2p5D, Layer, Space};
use alife::world::{Params, World};

#[test]
fn central_source_builds_a_center_rich_gradient() {
    let (w, h) = (32u32, 32u32);
    let space = Grid2p5D::new(w, h);
    let params = Params {
        decay: 0.0,
        ..Default::default()
    };
    let mut world = World::new(space, params);
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));

    for _ in 0..200 {
        world.step();
    }

    let idx = |x, y| world.space.index(Coord::new(x, y, Layer::Surface));
    let center = world.valaar.get(idx(w / 2, h / 2));
    let edge = world.valaar.get(idx(0, 0));
    assert!(center > edge, "center {center} should exceed edge {edge}");
    assert!(edge >= 0.0, "valaar must stay non-negative");
    assert!(world.valaar.total() > 0.0, "world should hold valaar");
}
