//! The ecology loop as a set of pure-ish tick functions over the substrate
//! (`Space`/`Field`) and the `Population`. No hidden state; ordering lives in
//! `Sim::step`. Selection is implicit — nothing here scores fitness.
//!
//! These functions are the *trait-vector clade's* ecology: they read genome
//! traits directly (`diet`, `size`, …). The clade-agnostic lifecycle methods
//! (`max_energy`, `is_alive`, …) come from the `Organism` trait.

use crate::field::Field;
use crate::organism::Organism;
use crate::params::EcoParams;
use crate::population::Population;
use crate::space::Space;

/// Autotrophy: each organism with an autotroph fraction `(1 - diet)` draws
/// valaar from the cell it stands in, scaled by `valaar_efficiency`, capped by
/// what's present and by remaining storage. The drawn valaar leaves the field.
pub fn absorb<S: Space>(space: &S, field: &mut Field, pop: &mut Population, eco: &EcoParams) {
    for o in pop.organisms_mut() {
        let auto = 1.0 - o.genome.diet;
        if auto <= 0.0 {
            continue;
        }
        let i = space.index(o.pos);
        let avail = field.get(i);
        if avail <= 0.0 {
            continue;
        }
        let room = (o.max_energy(eco) - o.energy).max(0.0);
        let want = eco.uptake_rate * o.genome.valaar_efficiency * auto * avail;
        let gain = want.min(avail).min(room);
        field.add(i, -gain);
        o.energy += gain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::organism::{Organism, TraitOrganism};
    use crate::params::EcoParams;
    use crate::population::Population;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    // [size, valaar_efficiency, speed, diet, repro_threshold, lifespan]
    fn genome(diet: f32, eff: f32) -> Genome {
        Genome::from_array([0.5, eff, 0.0, diet, 0.9, 0.5])
    }

    #[test]
    fn autotroph_absorbs_and_conserves() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(1, 1, Layer::Surface);
        field.set(space.index(c), 10.0);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(genome(0.0, 1.0), c, 1.0)); // pure autotroph

        let field_before = field.total();
        let energy_before = pop.organisms()[0].energy;
        absorb(&space, &mut field, &mut pop, &eco);
        let gained = pop.organisms()[0].energy - energy_before;
        let lost = field_before - field.total();

        assert!(gained > 0.0, "autotroph should gain energy");
        assert!((gained - lost).abs() < 1e-5, "valaar must be conserved");
    }

    #[test]
    fn pure_predator_absorbs_nothing() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(1, 1, Layer::Surface);
        field.set(space.index(c), 10.0);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(genome(1.0, 1.0), c, 1.0)); // pure predator

        absorb(&space, &mut field, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 1.0);
        assert_eq!(field.total(), 10.0);
    }

    #[test]
    fn absorption_is_capped_by_storage() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(1, 1, Layer::Surface);
        field.set(space.index(c), 1000.0);
        let mut pop = Population::new();
        let o = TraitOrganism::new(genome(0.0, 1.0), c, 0.0);
        let cap = o.max_energy(&eco);
        pop.spawn(o);

        absorb(&space, &mut field, &mut pop, &eco);
        assert!(pop.organisms()[0].energy <= cap + 1e-5, "must not exceed storage");
    }
}
