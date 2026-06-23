//! Organisms and the shared `Organism` contract.
//!
//! `Organism` is the clade-agnostic interface every kind of life implements. It
//! knows only the *universal* facts — where it is, its stored valaar (`energy`),
//! its `age`, whether it is still alive, and its metabolic limits. It says
//! NOTHING about how a genome is represented, so future clades
//! (`NeuralOrganism`, `MorphologicalOrganism`, …) can implement it without this
//! trait changing.
//!
//! `TraitOrganism` is the first clade: behaviour and metabolism are driven by a
//! flat vector of trait scalars (`Genome`).

use crate::genome::Genome;
use crate::params::EcoParams;
use crate::space::Coord;

/// Genetic threshold above which an organism is *able* to swim.
const SWIM_THRESHOLD: f32 = 0.5;

/// What every organism — of any clade — must provide. Deliberately general:
/// only the shared lifecycle, never a genome representation.
pub trait Organism {
    /// Where it currently is.
    fn pos(&self) -> Coord;
    /// Move it to another cell.
    fn set_pos(&mut self, pos: Coord);
    /// Stored valaar — the single energy currency.
    fn energy(&self) -> f32;
    /// Overwrite stored energy.
    fn set_energy(&mut self, energy: f32);
    /// Ticks lived so far.
    fn age(&self) -> u32;
    /// Advance age by one tick.
    fn advance_age(&mut self);

    /// Maximum energy this body can store.
    fn max_energy(&self, eco: &EcoParams) -> f32;
    /// Energy spent merely staying alive each tick.
    fn basal_cost(&self, eco: &EcoParams) -> f32;
    /// Maximum age in ticks before death of old age.
    fn lifespan_ticks(&self, eco: &EcoParams) -> u32;

    /// Alive iff it still has energy and has not outlived its lifespan. Shared
    /// by every clade — defined from the primitives above.
    fn is_alive(&self, eco: &EcoParams) -> bool {
        self.energy() > 0.0 && self.age() < self.lifespan_ticks(eco)
    }

    /// Add (or, when negative, spend) energy. Convenience for the ecology loop.
    fn add_energy(&mut self, delta: f32) {
        self.set_energy(self.energy() + delta);
    }
}

/// The trait-vector clade: a `Genome` of scalar traits drives reflex behaviour
/// and metabolism. The first organism type; others arrive as new clades.
#[derive(Clone, Debug)]
pub struct TraitOrganism {
    pub genome: Genome,
    pub pos: Coord,
    /// Stored valaar. This *is* the energy currency.
    pub energy: f32,
    pub age: u32,
}

impl TraitOrganism {
    pub fn new(genome: Genome, pos: Coord, energy: f32) -> Self {
        TraitOrganism { genome, pos, energy, age: 0 }
    }

    /// Whether this organism can tunnel through Valaar. Purely genetic
    /// (Darwinian): able iff `genome.swim > SWIM_THRESHOLD`.
    pub fn can_swim(&self) -> bool {
        self.genome.swim > SWIM_THRESHOLD
    }
}

impl Organism for TraitOrganism {
    fn pos(&self) -> Coord {
        self.pos
    }

    fn set_pos(&mut self, pos: Coord) {
        self.pos = pos;
    }

    fn energy(&self) -> f32 {
        self.energy
    }

    fn set_energy(&mut self, energy: f32) {
        self.energy = energy;
    }

    fn age(&self) -> u32 {
        self.age
    }

    fn advance_age(&mut self) {
        self.age += 1;
    }

    fn max_energy(&self, eco: &EcoParams) -> f32 {
        eco.base_energy + self.genome.size * eco.size_energy
    }

    fn basal_cost(&self, eco: &EcoParams) -> f32 {
        eco.basal_cost + self.genome.size * eco.size_cost
    }

    fn lifespan_ticks(&self, eco: &EcoParams) -> u32 {
        let span = (eco.max_lifespan - eco.min_lifespan) as f32;
        eco.min_lifespan + (self.genome.lifespan * span) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::space::{Coord, Layer};

    fn genome_with(size: f32, lifespan: f32) -> Genome {
        Genome::from_array([size, 0.5, 0.5, 0.5, 0.5, lifespan, 0.5, 0.5, 0.5, 1.0, 0.0, 0.0])
    }

    #[test]
    fn bigger_body_stores_and_costs_more() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let small = TraitOrganism::new(genome_with(0.0, 0.5), pos, 1.0);
        let big = TraitOrganism::new(genome_with(1.0, 0.5), pos, 1.0);
        assert!(big.max_energy(&eco) > small.max_energy(&eco));
        assert!(big.basal_cost(&eco) > small.basal_cost(&eco));
    }

    #[test]
    fn lifespan_ticks_span_the_configured_range() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let young = TraitOrganism::new(genome_with(0.5, 0.0), pos, 1.0);
        let old = TraitOrganism::new(genome_with(0.5, 1.0), pos, 1.0);
        assert_eq!(young.lifespan_ticks(&eco), eco.min_lifespan);
        assert_eq!(old.lifespan_ticks(&eco), eco.max_lifespan);
    }

    #[test]
    fn is_alive_tracks_energy_and_age() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let mut o = TraitOrganism::new(genome_with(0.5, 0.0), pos, 1.0);
        assert!(o.is_alive(&eco)); // young, fed
        o.energy = 0.0;
        assert!(!o.is_alive(&eco)); // starved
        o.energy = 1.0;
        o.age = o.lifespan_ticks(&eco);
        assert!(!o.is_alive(&eco)); // too old
    }

    #[test]
    fn usable_through_the_organism_trait() {
        // The ecology loop reads organisms through the trait; prove a clade
        // works behind `&mut dyn Organism` and the shared lifecycle reads through.
        let eco = EcoParams::default();
        let pos = Coord::new(1, 2, Layer::Surface);
        let mut o = TraitOrganism::new(genome_with(0.5, 1.0), pos, 2.0);
        let any: &mut dyn Organism = &mut o;
        assert_eq!(any.pos(), pos);
        any.add_energy(1.0);
        assert_eq!(any.energy(), 3.0);
        any.advance_age();
        assert_eq!(any.age(), 1);
        assert!(any.is_alive(&eco));
    }

    fn swim_genome(swim: f32) -> Genome {
        Genome::from_array([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, swim, 1.0, 0.0, 0.0])
    }

    #[test]
    fn can_swim_requires_gene_above_threshold() {
        let pos = Coord::new(0, 0, Layer::Surface);
        assert!(TraitOrganism::new(swim_genome(0.9), pos, 1.0).can_swim());
        assert!(!TraitOrganism::new(swim_genome(0.4), pos, 1.0).can_swim());
    }

}
