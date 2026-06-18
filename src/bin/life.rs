//! Headless life census: warm the field, seed autotrophs, run, and print the
//! population and mean genome every so often. No graphics (a later plan).

use alife::genome::Genome;
use alife::organism::TraitOrganism;
use alife::params::EcoParams;
use alife::rng::Rng;
use alife::sim::Sim;
use alife::space::{Coord, Grid2p5D, Layer};
use alife::world::{Params, World};

fn main() {
    let (w, h) = (48u32, 48u32);
    let space = Grid2p5D::new(w, h);
    let mut world = World::new(space, Params::default());
    world.add_source(Coord::new(w / 2, h / 2, Layer::Surface));
    world.add_access_point(w / 2, h / 2);

    let mut sim = Sim::new(world, EcoParams::default(), 1234);
    for _ in 0..150 {
        sim.world.step();
    }

    let mut rng = Rng::new(99);
    for _ in 0..120 {
        let x = (rng.next_unit() * w as f32) as u32;
        let y = (rng.next_unit() * h as f32) as u32;
        let pos = Coord::new(x.min(w - 1), y.min(h - 1), Layer::Surface);
        let g = Genome::from_array([0.4, 0.7, 0.3, 0.2, 0.5, 0.6]);
        sim.seed_organism(TraitOrganism::new(g, pos, sim.eco.initial_energy));
    }

    println!("tick   pop   mean_size mean_diet mean_eff");
    for tick in 0..=600 {
        if tick % 50 == 0 {
            let n = sim.pop.len().max(1) as f32;
            let mut s = 0.0;
            let mut d = 0.0;
            let mut e = 0.0;
            for o in sim.pop.organisms() {
                s += o.genome.size;
                d += o.genome.diet;
                e += o.genome.valaar_efficiency;
            }
            println!(
                "{tick:>4}  {:>4}   {:>8.3} {:>8.3} {:>8.3}",
                sim.pop.len(),
                s / n,
                d / n,
                e / n,
            );
        }
        sim.step();
    }
}
