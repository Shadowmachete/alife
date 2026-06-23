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
use crate::quakes::Quakes;
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
    /// Optional Vraze earthquakes that burst reservoir valaar up to the surface
    /// (terrain path only; `None` headless).
    quakes: Option<Quakes>,
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
            quakes: None,
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

    /// Attach earthquakes (call once, after construction).
    pub fn set_quakes(&mut self, quakes: Quakes) {
        self.quakes = Some(quakes);
    }

    /// Surface cells of currently-fracturing pools (viewer overlay); empty if no
    /// quakes are attached.
    pub fn quake_active_cells(&self) -> Vec<(u32, u32)> {
        self.quakes
            .as_ref()
            .map(|q| q.active_cells())
            .unwrap_or_default()
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

        // Vraze earthquakes: burst exposed reservoir valaar up to the surface,
        // before feeding so life can absorb the pulse this tick.
        if let Some(quakes) = self.quakes.as_mut() {
            quakes.update(&self.calendar, &self.world.space, &mut self.world.valaar);
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
        );
        ecology::predate(&self.world.space, &mut self.pop, &self.eco);
        ecology::environmental_stress(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::substitute_feed(&self.world.space, &self.heat, &self.water, &mut self.pop, &self.eco);
        ecology::metabolize(&mut self.pop, &self.eco);
        ecology::cull_and_recycle(&self.world.space, &mut self.world.valaar, &mut self.pop, &self.eco);
        ecology::reproduce(&mut self.pop, &self.eco, &mut self.rng, season);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::season::CRAWS_PER_YEAR;
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

    #[test]
    fn a_quake_pulses_surface_valaar_in_vraze() {
        use crate::quakes::{QuakeConfig, Quakes, ReservoirPool};
        use crate::space::{Coord, Layer};

        // 1x1 surface+underground world; a reservoir pool under (0,0).
        let mut world = World::new(Grid2p5D::new(1, 1), Params::default());
        let ui = world.space.index(Coord::new(0, 0, Layer::Underground));
        let si = world.space.index(Coord::new(0, 0, Layer::Surface));
        world.add_source(Coord::new(0, 0, Layer::Underground)); // pool refills below
        let mut sim = Sim::new(world, EcoParams::default(), 1);

        let cfg = QuakeConfig {
            erupt_fraction: 1.0,
            release_fraction: 1.0,
            ..QuakeConfig::default()
        };
        sim.set_quakes(Quakes::new(vec![ReservoirPool { cells: vec![(0, 0)] }], cfg, 99));

        // Run a full year; the surface cell must receive a burst during Vraze
        // (it is 0 otherwise — nothing else feeds the surface here).
        let mut max_surface = 0.0f32;
        for _ in 0..CRAWS_PER_YEAR {
            sim.step();
            max_surface = max_surface.max(sim.world.valaar.get(si));
        }
        assert!(max_surface > 0.0, "a Vraze quake pulsed valaar to the surface");
        let _ = ui; // underground accumulates between quakes
    }

    #[test]
    fn a_generalist_outlasts_a_specialist_in_a_warm_valaar_desert() {
        use crate::genome::Genome;
        use crate::space::{Coord, Layer};

        // No valaar anywhere, but a hot surface: only a heat-generalist offsets
        // upkeep. The two organisms are identical except for valaar_reliance.
        let world = World::new(Grid2p5D::new(1, 1), Params::default());
        let mut sim = Sim::new(world, EcoParams::default(), 1);
        for i in 0..sim.heat.len() {
            sim.heat.set(i, 1.0);
        }
        let at = Coord::new(0, 0, Layer::Surface);
        // indices 9/10/11 = valaar_reliance / heat_affinity / water_affinity; speed 0.
        let generalist =
            Genome::from_array([0.0, 1.0, 0.0, 0.0, 0.9, 1.0, 1.0, 0.5, 0.5, 0.0, 1.0, 0.0]);
        let specialist =
            Genome::from_array([0.0, 1.0, 0.0, 0.0, 0.9, 1.0, 1.0, 0.5, 0.5, 1.0, 1.0, 0.0]);
        sim.seed_organism(TraitOrganism::new(generalist, at, 2.0));
        sim.seed_organism(TraitOrganism::new(specialist, at, 2.0));
        let energy = |s: &Sim<Grid2p5D>, idx: usize| s.pop.organisms()[idx].energy;
        for _ in 0..3 {
            sim.step();
        }
        assert!(
            energy(&sim, 0) > energy(&sim, 1),
            "heat-generalist offsets upkeep; specialist does not"
        );
    }
}
