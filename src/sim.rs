//! `Sim` ties the substrate (`World`) to life (`Population`) and runs the tick
//! in a fixed order. It owns the single `Rng` so the whole simulation is
//! reproducible from `(seed, params, seeds)`.

use crate::ecology;
use crate::organism::TraitOrganism;
use crate::params::EcoParams;
use crate::population::Population;
use crate::rng::Rng;
use crate::space::Space;
use crate::world::World;

pub struct Sim<S: Space> {
    pub world: World<S>,
    pub pop: Population,
    pub eco: EcoParams,
    pub rng: Rng,
}

impl<S: Space> Sim<S> {
    pub fn new(world: World<S>, eco: EcoParams, seed: u64) -> Self {
        Sim {
            world,
            pop: Population::new(),
            eco,
            rng: Rng::new(seed),
        }
    }

    /// Current number of living organisms.
    pub fn population(&self) -> usize {
        self.pop.len()
    }

    /// Place an organism into the world (used for the initial seeding).
    pub fn seed_organism(&mut self, o: TraitOrganism) {
        self.pop.spawn(o);
    }

    /// Advance one tick: substrate, then the ecology loop in fixed order.
    pub fn step(&mut self) {
        self.world.step();
        ecology::absorb(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
        );
        ecology::predate(&self.world.space, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
        ecology::cull_and_recycle(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::reproduce(&mut self.pop, &self.eco, &mut self.rng);
    }
}
