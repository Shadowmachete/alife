//! The ecology loop as a set of pure-ish tick functions over the substrate
//! (`Space`/`Field`) and the `Population`. No hidden state; ordering lives in
//! `Sim::step`. Selection is implicit — nothing here scores fitness.
//!
//! These functions are the *trait-vector clade's* ecology: they read genome
//! traits directly (`diet`, `size`, …). The clade-agnostic lifecycle methods
//! (`max_energy`, `is_alive`, …) come from the `Organism` trait.

use crate::field::Field;
use crate::organism::{Organism, TraitOrganism};
use crate::params::EcoParams;
use crate::population::Population;
use crate::rng::Rng;
use crate::season::Season;
use crate::space::{Coord, Space};

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

/// Step in direction `(dx, dy)` from `from` across a contiguous run of
/// `swimmable` (Valaar) cells and return the first walkable land cell beyond the
/// band together with the band width. `None` if the immediate neighbour isn't
/// Valaar, the band runs off the map, or the far side isn't walkable land.
fn tunnel_exit<S: Space>(
    space: &S,
    passable: Option<&[bool]>,
    swimmable: Option<&[bool]>,
    from: Coord,
    dx: i32,
    dy: i32,
) -> Option<(Coord, u32)> {
    let mut x = from.x as i64;
    let mut y = from.y as i64;
    let mut width = 0u32;
    loop {
        x += dx as i64;
        y += dy as i64;
        if x < 0 || y < 0 {
            return None;
        }
        let c = Coord::new(x as u32, y as u32, from.layer);
        if !space.in_bounds(c) {
            return None;
        }
        let i = space.index(c);
        let is_valaar = match swimmable {
            Some(m) => m[i],
            None => false,
        };
        if is_valaar {
            width += 1;
            continue;
        }
        // First non-Valaar cell beyond the band.
        if width == 0 {
            return None; // immediate neighbour wasn't Valaar -> not a crossing
        }
        let open = match passable {
            Some(m) => m[i],
            None => true,
        };
        return if open { Some((c, width)) } else { None };
    }
}

/// Each organism moves with probability `speed` toward its richest in-bounds,
/// walkable planar neighbour (gradient ascent on valaar). Moving costs
/// `move_cost·speed`. `passable`: `None` = no terrain constraint; otherwise a
/// cell is walkable only where `passable[index]`. A **tunneller** (`can_swim`)
/// may additionally *teleport straight through* a contiguous band of `swimmable`
/// (Valaar) cells to the first walkable land cell on the far side, paying an
/// extra `valaar_drain` per Valaar cell crossed. Neighbours never cross layers.
pub fn move_organisms<S: Space>(
    space: &S,
    field: &Field,
    pop: &mut Population,
    eco: &EcoParams,
    rng: &mut Rng,
    passable: Option<&[bool]>,
    swimmable: Option<&[bool]>,
) {
    for o in pop.organisms_mut() {
        // Draw first so the rng stream advances once per organism regardless.
        if rng.next_unit() >= o.genome.speed {
            continue;
        }
        let mut best = o.pos;
        let mut best_v = field.get(space.index(o.pos));
        let mut best_width = 0u32; // Valaar cells crossed to reach `best` (0 = a walk)
        // Walkable planar neighbours (ordinary gradient ascent).
        for n in space.planar_neighbors(o.pos) {
            let ni = space.index(n);
            let open = match passable {
                Some(m) => m[ni],
                None => true,
            };
            if !open {
                continue; // impassable terrain blocks the step
            }
            let v = field.get(ni);
            if v > best_v {
                best_v = v;
                best = n;
                best_width = 0;
            }
        }
        // Tunnellers can teleport straight across a Valaar band to the far bank.
        if o.can_swim() {
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                if let Some((exit, width)) = tunnel_exit(space, passable, swimmable, o.pos, dx, dy) {
                    let v = field.get(space.index(exit));
                    if v > best_v {
                        best_v = v;
                        best = exit;
                        best_width = width;
                    }
                }
            }
        }
        if best != o.pos {
            o.pos = best;
            o.energy -= eco.move_cost * o.genome.speed;
            o.energy -= eco.valaar_drain * best_width as f32; // 0 for a plain walk
        }
    }
}

/// Resolve at most one predation per cell: the strongest predator
/// (`size·diet`, ties→lowest index) eats the smallest other occupant, but only
/// if it is a real predator (`diet > 0.5`) and strictly bigger than its victim.
/// Prey energy is drained to zero (it dies next cull); the predator banks
/// `prey.energy · predation_efficiency · valaar_efficiency`, capped by storage.
pub fn predate<S: Space>(space: &S, pop: &mut Population, eco: &EcoParams) {
    let occ = pop.occupancy(space);
    let orgs = pop.organisms_mut();
    for cell in &occ {
        if cell.len() < 2 {
            continue;
        }
        // Strongest attacker by power = size * diet (ties → lowest index).
        let mut attacker = cell[0];
        for &i in cell {
            let pi = orgs[i].genome.size * orgs[i].genome.diet;
            let pa = orgs[attacker].genome.size * orgs[attacker].genome.diet;
            if pi > pa {
                attacker = i;
            }
        }
        if orgs[attacker].genome.diet <= 0.5 {
            continue; // no real predator here
        }
        // Smallest victim among the others (ties → lowest index).
        let mut victim: Option<usize> = None;
        for &i in cell {
            if i == attacker {
                continue;
            }
            match victim {
                None => victim = Some(i),
                Some(v) if orgs[i].genome.size < orgs[v].genome.size => victim = Some(i),
                _ => {}
            }
        }
        let victim = match victim {
            Some(v) => v,
            None => continue,
        };
        if orgs[attacker].genome.size <= orgs[victim].genome.size {
            continue; // can't overpower
        }
        let prey_energy = orgs[victim].energy;
        let gain = prey_energy * eco.predation_efficiency * orgs[attacker].genome.valaar_efficiency;
        orgs[victim].energy = 0.0;
        let cap = orgs[attacker].max_energy(eco);
        orgs[attacker].energy = (orgs[attacker].energy + gain).min(cap);
    }
}

/// Remove organisms standing on any of `drowned` (surface-plane cell indices) —
/// e.g. when a land bridge sinks back to ocean beneath them.
pub fn drown<S: Space>(space: &S, pop: &mut Population, drowned: &[usize]) {
    if drowned.is_empty() {
        return;
    }
    pop.retain(|o| !drowned.contains(&space.index(o.pos)));
}

/// Per-organism mutation magnitude at birth: the base `mutation_rate`, multiplied
/// by `rasgun_mutation_mult` during the Rasgun surge and left flat otherwise.
pub fn mutation_rate(eco: &EcoParams, season: Season) -> f32 {
    let season_mult = if season == Season::Rasgun { eco.rasgun_mutation_mult } else { 1.0 };
    eco.mutation_rate * season_mult
}

/// Asexual reproduction: any organism at or above its energy threshold spawns
/// one child in its own cell, taking `repro_cost_fraction` of the parent's
/// energy and a mutated copy of its genome. The mutation magnitude spikes during
/// Rasgun (`mutation_rate`). Children are collected first, then appended, so
/// iteration order (determinism) is stable.
pub fn reproduce(pop: &mut Population, eco: &EcoParams, rng: &mut Rng, season: Season) {
    let mut children: Vec<TraitOrganism> = Vec::new();
    for o in pop.organisms_mut() {
        let threshold = o.genome.repro_threshold * o.max_energy(eco);
        if o.energy >= threshold && o.energy > 0.0 {
            let child_energy = o.energy * eco.repro_cost_fraction;
            o.energy -= child_energy;
            let rate = mutation_rate(eco, season);
            let child_genome = o.genome.mutate(rng, rate);
            children.push(TraitOrganism::new(child_genome, o.pos, child_energy));
        }
    }
    for c in children {
        pop.spawn(c);
    }
}

/// Drain energy from organisms whose cell is hotter or drier than their genes
/// can stand. Heat above `heat_tolerance` and water below the organism's need
/// (`1 - drought_tolerance`) each cost energy. Never adds energy; deaths fall
/// out of the normal cull.
pub fn environmental_stress<S: Space>(
    space: &S,
    heat: &Field,
    water: &Field,
    pop: &mut Population,
    eco: &EcoParams,
) {
    for o in pop.organisms_mut() {
        let i = space.index(o.pos);
        let heat_excess = (heat.get(i) - o.genome.heat_tolerance).max(0.0);
        let water_need = 1.0 - o.genome.drought_tolerance;
        let water_deficit = (water_need - water.get(i)).max(0.0);
        let penalty = eco.heat_stress * heat_excess + eco.drought_stress * water_deficit;
        o.energy -= penalty;
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

    // [size, valaar_efficiency, speed, diet, repro_threshold, lifespan, heat_tol, drought_tol]
    fn genome(diet: f32, eff: f32) -> Genome {
        Genome::from_array([0.5, eff, 0.0, diet, 0.9, 0.5, 0.5, 0.5, 0.5])
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
        pop.spawn(TraitOrganism::new(Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5]), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, None, None);
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
        pop.spawn(TraitOrganism::new(Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5]), peak, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, None, None);
        assert_eq!(pop.organisms()[0].pos, peak);
        assert_eq!(pop.organisms()[0].energy, 5.0, "no move, no cost");
    }

    #[test]
    fn does_not_step_onto_impassable_richer_neighbor() {
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        for x in 0..4u32 {
            field.set(space.index(Coord::new(x, 0, Layer::Surface)), x as f32);
        }
        let mut mask = vec![true; space.len()];
        mask[space.index(Coord::new(3, 0, Layer::Surface))] = false; // richest cell barred
        let start = Coord::new(2, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5]),
            start,
            5.0,
        ));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&mask), None);
        assert_eq!(pop.organisms()[0].pos, start, "must not enter an impassable cell");
    }

    #[test]
    fn boxed_in_organism_stays_and_pays_nothing() {
        let space = Grid2p5D::new(3, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        for x in 0..3u32 {
            field.set(space.index(Coord::new(x, 0, Layer::Surface)), x as f32);
        }
        let mut mask = vec![true; space.len()];
        mask[space.index(Coord::new(0, 0, Layer::Surface))] = false;
        mask[space.index(Coord::new(2, 0, Layer::Surface))] = false;
        let center = Coord::new(1, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(
            Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5]),
            center,
            5.0,
        ));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&mask), None);
        assert_eq!(pop.organisms()[0].pos, center);
        assert_eq!(pop.organisms()[0].energy, 5.0, "no move, no cost");
    }

    // [size, eff, speed, diet, repro, lifespan, heat_tol, drought_tol, swim]
    fn swimmer(swim: f32) -> Genome {
        Genome::from_array([0.5, 1.0, 1.0, 0.0, 0.9, 0.5, 0.5, 0.5, swim])
    }

    /// land(0) - V(1) - V(2) - land(3): a 2-wide Valaar band with land on both
    /// banks. `far` sets the far-bank field value. Returns the masks too.
    fn river_row(far: f32) -> (Grid2p5D, crate::field::Field, Vec<bool>, Vec<bool>) {
        let space = Grid2p5D::new(4, 1);
        let mut field = crate::field::Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 1.0); // near bank
        field.set(space.index(Coord::new(3, 0, Layer::Surface)), far); // far bank
        let mut passable = vec![true; space.len()];
        let mut swimmable = vec![false; space.len()];
        for x in 1..3u32 {
            let i = space.index(Coord::new(x, 0, Layer::Surface));
            passable[i] = false; // Valaar impassable to walkers
            swimmable[i] = true;
        }
        (space, field, passable, swimmable)
    }

    #[test]
    fn tunneller_teleports_across_band_to_far_bank() {
        let (space, field, passable, swimmable) = river_row(9.0); // far bank richer
        let eco = EcoParams::default();
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.9), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), Some(&swimmable));
        assert_eq!(
            pop.organisms()[0].pos,
            Coord::new(3, 0, Layer::Surface),
            "a tunneller crosses straight to the far bank, never onto Valaar"
        );
        let expected = 5.0 - eco.move_cost - eco.valaar_drain * 2.0;
        assert!(
            (pop.organisms()[0].energy - expected).abs() < 1e-6,
            "pays move cost + valaar_drain per cell crossed"
        );
    }

    #[test]
    fn non_tunneller_cannot_cross_the_band() {
        let (space, field, passable, swimmable) = river_row(9.0);
        let eco = EcoParams::default();
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.1), start, 5.0)); // gene below threshold
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), Some(&swimmable));
        assert_eq!(pop.organisms()[0].pos, start, "a non-tunneller is stuck on its bank");
    }

    #[test]
    fn tunneller_stays_if_far_bank_is_poorer() {
        let (space, field, passable, swimmable) = river_row(0.5); // far bank poorer than near
        let eco = EcoParams::default();
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.9), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), Some(&swimmable));
        assert_eq!(pop.organisms()[0].pos, start, "no incentive to cross to a poorer bank");
    }

    #[test]
    fn no_landing_beyond_band_blocks_the_crossing() {
        // land(0) - V(1) - V(2) - ocean(3): the far side is a barrier, not land.
        let space = Grid2p5D::new(4, 1);
        let eco = EcoParams::default();
        let mut field = crate::field::Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 1.0);
        field.set(space.index(Coord::new(3, 0, Layer::Surface)), 99.0); // tempting but unreachable
        let mut passable = vec![true; space.len()];
        let mut swimmable = vec![false; space.len()];
        for x in 1..3u32 {
            let i = space.index(Coord::new(x, 0, Layer::Surface));
            passable[i] = false;
            swimmable[i] = true;
        }
        passable[space.index(Coord::new(3, 0, Layer::Surface))] = false; // ocean
        let start = Coord::new(0, 0, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(swimmer(0.9), start, 5.0));
        let mut rng = Rng::new(1);
        move_organisms(&space, &field, &mut pop, &eco, &mut rng, Some(&passable), Some(&swimmable));
        assert_eq!(pop.organisms()[0].pos, start, "cannot tunnel into a non-land far side");
    }

    // [size, eff, speed, diet, repro_threshold, lifespan, heat_tol, drought_tol]
    fn predator(size: f32) -> Genome {
        Genome::from_array([size, 1.0, 0.0, 1.0, 0.9, 0.5, 0.5, 0.5, 0.5])
    }
    fn prey(size: f32) -> Genome {
        Genome::from_array([size, 1.0, 0.0, 0.0, 0.9, 0.5, 0.5, 0.5, 0.5])
    }

    #[test]
    fn predator_eats_smaller_co_located_prey() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(predator(0.9), c, 1.0)); // big predator
        pop.spawn(TraitOrganism::new(prey(0.2), c, 3.0)); // small prey, energy 3

        predate(&space, &mut pop, &eco);

        let pred = &pop.organisms()[0];
        let victim = &pop.organisms()[1];
        assert!(pred.energy > 1.0, "predator should gain");
        assert_eq!(victim.energy, 0.0, "prey should be drained");
    }

    #[test]
    fn lone_organism_is_not_eaten() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(predator(0.9), c, 1.0));
        predate(&space, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 1.0);
    }

    #[test]
    fn autotrophs_do_not_predate() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(2, 2, Layer::Surface);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(prey(0.9), c, 1.0)); // big but diet 0
        pop.spawn(TraitOrganism::new(prey(0.2), c, 3.0));
        predate(&space, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 1.0);
        assert_eq!(pop.organisms()[1].energy, 3.0);
    }

    #[test]
    fn well_fed_organism_spawns_one_child() {
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut pop = Population::new();
        // repro_threshold 0.0 => any positive energy triggers reproduction.
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5]);
        let parent = TraitOrganism::new(g, c, 5.0);
        pop.spawn(parent);
        let mut rng = Rng::new(3);
        reproduce(&mut pop, &eco, &mut rng, Season::Goscon);
        assert_eq!(pop.len(), 2);
        let child = &pop.organisms()[1];
        assert_eq!(child.pos, c);
        assert!((child.energy - 5.0 * eco.repro_cost_fraction).abs() < 1e-6);
        // parent paid for it
        assert!(pop.organisms()[0].energy < 5.0);
    }

    #[test]
    fn starving_organism_does_not_reproduce() {
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut pop = Population::new();
        // repro_threshold 1.0 => needs full storage; give it almost none.
        let g = Genome::from_array([0.5, 1.0, 0.0, 0.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
        pop.spawn(TraitOrganism::new(g, c, 0.1));
        let mut rng = Rng::new(3);
        reproduce(&mut pop, &eco, &mut rng, Season::Goscon);
        assert_eq!(pop.len(), 1);
    }

    #[test]
    fn rasgun_amplifies_mutation() {
        let eco = EcoParams::default();
        let normal = mutation_rate(&eco, Season::Goscon);
        let rasgun = mutation_rate(&eco, Season::Rasgun);
        assert!((normal - eco.mutation_rate).abs() < 1e-6, "off-season is the flat base rate");
        assert!((rasgun - eco.mutation_rate * eco.rasgun_mutation_mult).abs() < 1e-5);
    }

    #[test]
    fn drown_removes_organisms_on_sunk_cells() {
        let space = Grid2p5D::new(3, 1);
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(genome(0.0, 1.0), Coord::new(0, 0, Layer::Surface), 1.0));
        pop.spawn(TraitOrganism::new(genome(0.0, 1.0), Coord::new(1, 0, Layer::Surface), 1.0));
        let sunk = vec![space.index(Coord::new(1, 0, Layer::Surface))];
        drown(&space, &mut pop, &sunk);
        assert_eq!(pop.len(), 1);
        assert_eq!(pop.organisms()[0].pos, Coord::new(0, 0, Layer::Surface));
    }

    // genome with explicit tolerances: [size, eff, speed, diet, repro, lifespan, heat_tol, drought_tol]
    fn tol_genome(heat_tol: f32, drought_tol: f32) -> Genome {
        Genome::from_array([0.5, 1.0, 0.0, 0.0, 0.9, 0.5, heat_tol, drought_tol, 0.5])
    }

    #[test]
    fn heat_intolerant_loses_energy_in_a_hot_cell() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut heat = crate::field::Field::zeros(space.len());
        let water = crate::field::Field::zeros(space.len());
        heat.set(space.index(c), 1.0); // scorching
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(tol_genome(0.1, 1.0), c, 5.0)); // can't take heat, no drought issue

        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert!(pop.organisms()[0].energy < 5.0, "heat-intolerant should suffer");
    }

    #[test]
    fn heat_tolerant_is_unscathed() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let mut heat = crate::field::Field::zeros(space.len());
        let mut water = crate::field::Field::zeros(space.len());
        heat.set(space.index(c), 1.0);
        water.set(space.index(c), 1.0); // wet, so no drought stress either
        let mut pop = Population::new();
        pop.spawn(TraitOrganism::new(tol_genome(1.0, 1.0), c, 5.0)); // immune

        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert_eq!(pop.organisms()[0].energy, 5.0, "tolerant should be unscathed");
    }

    #[test]
    fn drought_drains_the_intolerant() {
        let space = Grid2p5D::new(4, 4);
        let eco = EcoParams::default();
        let c = Coord::new(1, 1, Layer::Surface);
        let heat = crate::field::Field::zeros(space.len()); // cool
        let water = crate::field::Field::zeros(space.len()); // bone dry (0.0)
        let mut pop = Population::new();
        // drought_tolerance 0.0 => needs water 1.0; finds 0.0 => big deficit
        pop.spawn(TraitOrganism::new(tol_genome(1.0, 0.0), c, 5.0));

        environmental_stress(&space, &heat, &water, &mut pop, &eco);
        assert!(pop.organisms()[0].energy < 5.0, "drought-intolerant should suffer");
    }
}
