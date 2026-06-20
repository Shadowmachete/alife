//! `Sim` ties the substrate (`World`) to life (`Population`) and runs the tick
//! in a fixed order. It owns the single `Rng` so the whole simulation is
//! reproducible from `(seed, params, seeds)`.

use crate::climate::{self, Climate};
use crate::ecology;
use crate::field::Field;
use crate::organism::TraitOrganism;
use crate::params::EcoParams;
use crate::population::Population;
use crate::rng::Rng;
use crate::season::{Calendar, Season};
use crate::space::Space;
use crate::world::World;

pub struct Sim<S: Space> {
    pub world: World<S>,
    pub pop: Population,
    pub eco: EcoParams,
    pub rng: Rng,
    pub heat: Field,
    pub water: Field,
    pub calendar: Calendar,
    pub climate: Climate,
    /// Un-multiplied Rasconne source rate, captured at construction so the
    /// per-season multiplier always scales the same base.
    base_source: f32,
}

impl<S: Space> Sim<S> {
    /// Build a sim with the default climate.
    pub fn new(world: World<S>, eco: EcoParams, seed: u64) -> Self {
        Sim::with_climate(world, eco, Climate::default(), seed)
    }

    /// Build a sim with a chosen climate.
    pub fn with_climate(world: World<S>, eco: EcoParams, climate: Climate, seed: u64) -> Self {
        let len = world.space.len();
        let base_source = world.params.source_rate;
        Sim {
            world,
            pop: Population::new(),
            eco,
            rng: Rng::new(seed),
            heat: Field::zeros(len),
            water: Field::zeros(len),
            calendar: Calendar::new(),
            climate,
            base_source,
        }
    }

    /// Current number of living organisms.
    pub fn population(&self) -> usize {
        self.pop.len()
    }

    /// The current season.
    pub fn season(&self) -> Season {
        self.calendar.season()
    }

    /// Place an organism into the world (used for the initial seeding).
    pub fn seed_organism(&mut self, o: TraitOrganism) {
        self.pop.spawn(o);
    }

    /// Advance one tick: calendar → season-coupled valaar → substrate → climate
    /// → ecology loop (with environmental stress before metabolism).
    pub fn step(&mut self) {
        self.calendar.advance();
        let season = self.calendar.season();

        // [A6] valaar abundance breathes with the season.
        self.world.params.source_rate =
            self.base_source * climate::target(&self.climate, season).valaar_mult;

        self.world.step();
        climate::apply_climate(&mut self.heat, &mut self.water, season, &self.climate);

        ecology::absorb(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
            self.world.passability(),
        );
        ecology::predate(&self.world.space, &mut self.pop, &self.eco);
        ecology::environmental_stress(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
        ecology::cull_and_recycle(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::reproduce(&mut self.pop, &self.eco, &mut self.rng);
    }
}
