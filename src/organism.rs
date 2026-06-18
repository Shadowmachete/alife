//! A single organism: its genome, where it is, how much valaar it has stored
//! (`energy`), and how old it is. Metabolic quantities are derived from the
//! genome through `EcoParams` so the genome stays pure data.

use crate::genome::Genome;
use crate::params::EcoParams;
use crate::space::Coord;

#[derive(Clone, Debug)]
pub struct Organism {
    pub genome: Genome,
    pub pos: Coord,
    /// Stored valaar. This *is* the energy currency.
    pub energy: f32,
    pub age: u32,
}

impl Organism {
    pub fn new(genome: Genome, pos: Coord, energy: f32) -> Self {
        Organism { genome, pos, energy, age: 0 }
    }

    /// Maximum energy this body can store.
    pub fn max_energy(&self, eco: &EcoParams) -> f32 {
        eco.base_energy + self.genome.size * eco.size_energy
    }

    /// Energy spent just staying alive this tick.
    pub fn basal_cost(&self, eco: &EcoParams) -> f32 {
        eco.basal_cost + self.genome.size * eco.size_cost
    }

    /// Maximum age in ticks (genome `lifespan` lerps min..max).
    pub fn lifespan_ticks(&self, eco: &EcoParams) -> u32 {
        let span = (eco.max_lifespan - eco.min_lifespan) as f32;
        eco.min_lifespan + (self.genome.lifespan * span) as u32
    }

    /// Alive iff it still has energy and has not outlived its lifespan.
    pub fn is_alive(&self, eco: &EcoParams) -> bool {
        self.energy > 0.0 && self.age < self.lifespan_ticks(eco)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::params::EcoParams;
    use crate::space::{Coord, Layer};

    fn genome_with(size: f32, lifespan: f32) -> Genome {
        Genome::from_array([size, 0.5, 0.5, 0.5, 0.5, lifespan])
    }

    #[test]
    fn bigger_body_stores_and_costs_more() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let small = Organism::new(genome_with(0.0, 0.5), pos, 1.0);
        let big = Organism::new(genome_with(1.0, 0.5), pos, 1.0);
        assert!(big.max_energy(&eco) > small.max_energy(&eco));
        assert!(big.basal_cost(&eco) > small.basal_cost(&eco));
    }

    #[test]
    fn lifespan_ticks_span_the_configured_range() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let young = Organism::new(genome_with(0.5, 0.0), pos, 1.0);
        let old = Organism::new(genome_with(0.5, 1.0), pos, 1.0);
        assert_eq!(young.lifespan_ticks(&eco), eco.min_lifespan);
        assert_eq!(old.lifespan_ticks(&eco), eco.max_lifespan);
    }

    #[test]
    fn is_alive_tracks_energy_and_age() {
        let eco = EcoParams::default();
        let pos = Coord::new(0, 0, Layer::Surface);
        let mut o = Organism::new(genome_with(0.5, 0.0), pos, 1.0);
        assert!(o.is_alive(&eco)); // young, fed
        o.energy = 0.0;
        assert!(!o.is_alive(&eco)); // starved
        o.energy = 1.0;
        o.age = o.lifespan_ticks(&eco);
        assert!(!o.is_alive(&eco)); // too old
    }
}
