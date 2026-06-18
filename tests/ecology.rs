use alife::genome::Genome;
use alife::organism::TraitOrganism;
use alife::params::EcoParams;
use alife::sim::Sim;
use alife::space::{Coord, Grid2p5D, Layer};
use alife::world::{Params, World};

fn seeded_sim(seed: u64) -> Sim<Grid2p5D> {
    let (w, h) = (24u32, 24u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));

    let eco = EcoParams::default();
    let mut sim = Sim::new(world, eco, seed);

    // Warm the valaar field so there is food to seed life into.
    for _ in 0..100 {
        sim.world.step();
    }
    // Seed autotrophs in a patch around the source.
    let mut rng = alife::rng::Rng::new(seed ^ 0xABCD);
    for _ in 0..40 {
        let gx = (w / 2).saturating_sub(3) + (rng.next_unit() * 6.0) as u32;
        let gy = (h / 2).saturating_sub(3) + (rng.next_unit() * 6.0) as u32;
        let pos = Coord::new(gx.min(w - 1), gy.min(h - 1), Layer::Surface);
        // Low diet (autotroph), moderate everything else.
        let g = Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6]);
        sim.seed_organism(TraitOrganism::new(g, pos, sim.eco.initial_energy));
    }
    sim
}

#[test]
fn life_persists_under_the_gradient() {
    let mut sim = seeded_sim(7);
    assert_eq!(sim.population(), 40);
    for _ in 0..150 {
        sim.step();
    }
    assert!(sim.population() > 0, "the population went extinct");
}

#[test]
fn runs_are_deterministic() {
    let mut a = seeded_sim(11);
    let mut b = seeded_sim(11);
    for _ in 0..150 {
        a.step();
        b.step();
    }
    assert_eq!(a.population(), b.population(), "same seed must give same population");
}
