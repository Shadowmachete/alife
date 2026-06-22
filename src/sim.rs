//! `Sim` ties the substrate (`World`) to life (`Population`) and runs the tick
//! in a fixed order. It owns the single `Rng` so the whole simulation is
//! reproducible from `(seed, params, seeds)`.

use crate::bridges::Bridges;
use crate::climate::{self, Climate};
use crate::ecology;
use crate::field::Field;
use crate::organism::TraitOrganism;
use crate::params::EcoParams;
use crate::population::Population;
use crate::rng::Rng;
use crate::season::{Calendar, Season};
use crate::space::Space;
use crate::valaar::{self, ValaarPhase};
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
    /// Optional dynamic land bridges (terrain path only; `None` headless).
    bridges: Option<Bridges>,
    /// Un-multiplied Rasconne source rate, captured at construction so the
    /// per-season multiplier always scales the same base.
    base_source: f32,
    /// Un-multiplied decay, captured at construction so the per-phase multiplier
    /// always scales the same base.
    base_decay: f32,
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
        let base_decay = world.params.decay;
        Sim {
            world,
            pop: Population::new(),
            eco,
            rng: Rng::new(seed),
            heat: Field::zeros(len),
            water: Field::zeros(len),
            calendar: Calendar::new(),
            climate,
            bridges: None,
            base_source,
            base_decay,
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

    /// Attach dynamic land bridges (call once, after construction).
    pub fn set_bridges(&mut self, bridges: Bridges) {
        self.bridges = Some(bridges);
    }

    /// Advance one tick: calendar → season-coupled valaar → substrate → climate
    /// → ecology loop (with environmental stress before metabolism).
    pub fn step(&mut self) {
        self.calendar.advance();
        let season = self.calendar.season();
        let phase = ValaarPhase::for_season(season);
        let dynamics = phase.dynamics();

        // [A6] valaar abundance breathes with the season; its *phase* sets how
        // far it spreads (diffusion passes), how fast it drains (decay), and
        // whether it crystallises.
        let target = climate::target(&self.climate, season);
        self.world.params.source_rate = self.base_source * target.valaar_mult;
        self.world.params.diffuse_passes = dynamics.diffuse_passes;
        self.world.params.decay = self.base_decay * dynamics.decay_mult;

        self.world.step();
        valaar::freeze_thaw(&mut self.world.valaar, &mut self.world.crystal, phase, &dynamics);
        climate::apply_climate(&mut self.heat, &mut self.water, season, &self.climate);

        // Dynamic land bridges: open/close cells, drown anyone on a sunk cell.
        if let Some(bridges) = self.bridges.as_mut() {
            let upd = bridges.update(&self.calendar);
            for &i in &upd.opened {
                self.world.set_cell_passable(i, true);
            }
            for &i in &upd.closed {
                self.world.set_cell_passable(i, false);
            }
            ecology::drown(&self.world.space, &mut self.pop, &upd.closed);
        }

        ecology::absorb(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::move_organisms(
            &self.world.space,
            &self.world.valaar,
            &mut self.pop,
            &self.eco,
            &mut self.rng,
            self.world.passability(),
            self.world.swimmable(),
            self.world.descendable(),
        );
        ecology::predate(&self.world.space, &mut self.pop, &self.eco);
        ecology::environmental_stress(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
        ecology::cull_and_recycle(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::reproduce(&mut self.pop, &self.eco, &mut self.rng, season);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Grid2p5D, Space};
    use crate::world::{Params, World};

    #[test]
    fn crystal_builds_in_vraze_and_thaws_after() {
        let mut world = World::new(Grid2p5D::new(3, 3), Params::default());
        for i in 0..world.space.len() {
            world.valaar.set(i, 1.0); // give freezing something to grab everywhere
        }
        let mut sim = Sim::new(world, EcoParams::default(), 1);

        while sim.calendar.season() != Season::Vraze {
            sim.step();
        }
        let entering = sim.world.crystal.total();
        for _ in 0..30 {
            sim.step();
        }
        let in_vraze = sim.world.crystal.total();
        assert!(in_vraze > entering, "crystal accumulates during Vraze");

        while sim.calendar.season() == Season::Vraze {
            sim.step();
        }
        for _ in 0..50 {
            sim.step();
        }
        let after = sim.world.crystal.total();
        assert!(after < in_vraze, "crystal thaws once Vraze passes");
    }
}
