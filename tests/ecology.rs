use alife::genome::Genome;
use alife::organism::TraitOrganism;
use alife::params::EcoParams;
use alife::season::Season;
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
        let g = Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6, 0.6, 0.6, 0.0]);
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

#[test]
fn the_year_cycles_through_all_seasons() {
    let mut sim = seeded_sim(3);
    let mut seen: std::collections::HashSet<Season> = std::collections::HashSet::new();
    for _ in 0..alife::season::CRAWS_PER_YEAR {
        seen.insert(sim.season());
        sim.step();
    }
    // every arh should appear across a full year
    assert_eq!(seen.len(), 6, "all six seasons should occur in a year");
}

#[test]
fn drought_selects_for_drought_tolerance() {
    // A tiny world held permanently in Dansch (drought) by a custom climate:
    // every season targets the same bone-dry, hot environment.
    let space = Grid2p5D::new(8, 8);
    let mut world = World::new(space, Params::default());
    world.add_source(Coord::new(4, 4, Layer::Surface));
    let dry = alife::climate::SeasonTarget { heat: 0.9, water: 0.0, valaar_mult: 1.0 };
    let climate = alife::climate::Climate { targets: [dry; 6], relax_rate: 0.2 };
    let mut sim = Sim::with_climate(world, EcoParams::default(), climate, 1);
    for _ in 0..100 {
        sim.world.step();
    }
    // Two co-located lineages: one drought/heat tolerant, one not.
    for _ in 0..20 {
        let tough = Genome::from_array([0.3, 0.8, 0.1, 0.0, 0.4, 0.6, 0.9, 0.9, 0.0]);
        let frail = Genome::from_array([0.3, 0.8, 0.1, 0.0, 0.4, 0.6, 0.1, 0.1, 0.0]);
        sim.seed_organism(TraitOrganism::new(tough, Coord::new(4, 4, Layer::Surface), sim.eco.initial_energy));
        sim.seed_organism(TraitOrganism::new(frail, Coord::new(4, 4, Layer::Surface), sim.eco.initial_energy));
    }
    for _ in 0..200 {
        sim.step();
    }
    // Mean drought_tolerance should rise above the 0.5 we started balanced at.
    let n = sim.pop.len().max(1) as f32;
    let mean_dt: f32 = sim.pop.organisms().iter().map(|o| o.genome.drought_tolerance).sum::<f32>() / n;
    assert!(!sim.pop.is_empty(), "population went extinct");
    assert!(mean_dt > 0.5, "drought should have selected for tolerance, got {mean_dt}");
}
