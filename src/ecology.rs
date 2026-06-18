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
use crate::rng::Rng;
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

/// Spend basal energy, cap storage, and age every organism by one tick.
pub fn metabolize(pop: &mut Population, eco: &EcoParams) {
    for o in pop.organisms_mut() {
        o.energy -= o.basal_cost(eco);
        let cap = o.max_energy(eco);
        if o.energy > cap {
            o.energy = cap;
        }
        o.age += 1;
    }
}

/// Return each dead organism's remaining energy to its cell as detritus
/// (recycling), then drop the dead from the population.
pub fn cull_and_recycle<S: Space>(
    space: &S,
    field: &mut Field,
    pop: &mut Population,
    eco: &EcoParams,
) {
    for o in pop.organisms() {
        if !o.is_alive(eco) {
            let detritus = o.energy.max(0.0) * eco.detritus_fraction;
            if detritus > 0.0 {
                field.add(space.index(o.pos), detritus);
            }
        }
    }
    pop.retain(|o| o.is_alive(eco));
}

/// Each organism moves with probability `speed` toward its richest in-bounds
/// planar neighbour (gradient ascent on valaar). Moving costs `move_cost·speed`.
/// Neighbours never cross layers, so organisms stay on their layer.
pub fn move_organisms<S: Space>(
    space: &S,
    field: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
) {
    for o in pop.organisms_mut() {
        // Draw first so the rng stream advances once per organism regardless.
        if rng.next_unit() >= o.genome.speed {
            continue;
        }
        let mut best = o.pos;
        let mut best_v = field.get(space.index(o.pos));
        for n in space.planar_neighbors(o.pos) {
            let v = field.get(space.index(n));
            if v > best_v {
                best_v = v;
                best = n;
            }
        }
        if best != o.pos {
            o.pos = best;
            o.energy -= eco.move_cost * o.genome.speed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::organism::{Organism, TraitOrganism};
    use crate::params::EcoParams;
    use crate::population::Population;
    use crate::rng::Rng;
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

    #[test]
    fn metabolize_spends_energy_and_ages() {
        let eco = EcoParams::default();
        let c = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        // Start within storage capacity so the basal subtraction is visible
        // (max_energy for size 0.5 is 3.0; a higher seed would just clamp to 3.0).
        let o = TraitOrganism::new(genome(0.0, 1.0), c, 2.0);
        let cost = o.basal_cost(&eco);
        pop.spawn(o);
        metabolize(&mut pop, &eco);
        assert!((pop.organisms()[0].energy - (2.0 - cost)).abs() < 1e-6);
        assert_eq!(pop.organisms()[0].age, 1);
    }

    #[test]
    fn starved_organism_is_culled() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        let mut o = TraitOrganism::new(genome(0.0, 1.0), c, 0.0);
        o.energy = 0.0;
        pop.spawn(o);
        cull_and_recycle(&space, &mut field, &mut pop, &eco);
        assert_eq!(pop.len(), 0);
    }

    #[test]
    fn old_age_death_returns_detritus() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        let mut o = TraitOrganism::new(genome(0.0, 1.0), c, 4.0);
        o.age = o.lifespan_ticks(&eco); // too old, but still has energy
        let expected = 4.0 * eco.detritus_fraction;
        pop.spawn(o);
        cull_and_recycle(&space, &mut field, &mut pop, &eco);
        assert_eq!(pop.len(), 0);
        assert!((field.get(space.index(c)) - expected).abs() < 1e-6);
    }

    #[test]
    fn moves_uphill_toward_richer_valaar() {
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        // Increasing valaar to the right.
        for x in 0..4u32 {
            field.set(space.index(Coord::new(x, 0, Layer::Surface)), x as f32);
        }
        let start = Coord::new(1, 0, Layer::Surface);
        let mut pop = Population::new();
        // speed 1.0 => always moves.
        pop.spawn(TraitOrganism::new(Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5]), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng);
        assert_eq!(pop.organisms()[0].pos, Coord::new(2, 0, Layer::Surface));
        assert!(pop.organisms()[0].energy < 5.0, "moving costs energy");
    }

    #[test]
    fn at_local_max_it_stays_put() {
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        let peak = Coord::new(2, 0, Layer::Surface);
        field.set(space.index(peak), 100.0);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5]), peak, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng);
        assert_eq!(pop.organisms()[0].pos, peak);
        assert_eq!(pop.organisms()[0].energy, 5.0, "no move, no cost");
    }
}
