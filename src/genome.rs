//! The trait-vector genome. Every field is a scalar in `[0, 1]`; behaviour and
//! metabolism read these directly, and reproduction perturbs them. Designed so a
//! brain/morphology can be added later as a *new clade*, not a rewrite.

use crate::rng::Rng;

/// Number of scalar traits in the genome.
pub const TRAIT_COUNT: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Genome {
    /// Body size: more storage and predation power, but costlier metabolism.
    pub size: f32,
    /// Fraction of contacted valaar actually captured (absorption & predation).
    pub valaar_efficiency: f32,
    /// Movement tendency and per-move cost.
    pub speed: f32,
    /// Trophic axis: 0 = autotroph, 1 = predator.
    pub diet: f32,
    /// Reproduce once energy reaches this fraction of max storage.
    pub repro_threshold: f32,
    /// Maps (via EcoParams) to a maximum age in ticks.
    pub lifespan: f32,
    /// Resistance to heat stress (0 = none, 1 = immune within range).
    pub heat_tolerance: f32,
    /// Resistance to drought (high = needs little water).
    pub drought_tolerance: f32,
    /// Capacity to cross Valaar. Genetically "able to swim" when `> 0.5`; actual
    /// ability is further gated by the Lamarckian disuse counter on the organism
    /// (see `TraitOrganism::can_swim`).
    pub swim: f32,
    /// Reliance on valaar as a food: 1.0 = fully valaar-dependent (today's
    /// behaviour); lower lets a larger share of upkeep be met from substitutes,
    /// at the cost of weaker valaar extraction.
    pub valaar_reliance: f32,
    /// How well the organism harvests ambient heat to offset upkeep (thermotrophy).
    pub heat_affinity: f32,
    /// How well the organism harvests ambient water to offset upkeep (osmotrophy).
    pub water_affinity: f32,
}

impl Genome {
    pub fn from_array(a: [f32; TRAIT_COUNT]) -> Self {
        Genome {
            size: a[0],
            valaar_efficiency: a[1],
            speed: a[2],
            diet: a[3],
            repro_threshold: a[4],
            lifespan: a[5],
            heat_tolerance: a[6],
            drought_tolerance: a[7],
            swim: a[8],
            valaar_reliance: a[9],
            heat_affinity: a[10],
            water_affinity: a[11],
        }
    }

    pub fn to_array(&self) -> [f32; TRAIT_COUNT] {
        [
            self.size,
            self.valaar_efficiency,
            self.speed,
            self.diet,
            self.repro_threshold,
            self.lifespan,
            self.heat_tolerance,
            self.drought_tolerance,
            self.swim,
            self.valaar_reliance,
            self.heat_affinity,
            self.water_affinity,
        ]
    }

    /// Pin every trait into `[0, 1]`.
    pub fn clamped(self) -> Self {
        let mut a = self.to_array();
        for t in &mut a {
            *t = t.clamp(0.0, 1.0);
        }
        Genome::from_array(a)
    }

    /// A fresh genome with every trait drawn uniformly from `[0, 1)`.
    pub fn random(rng: &mut Rng) -> Self {
        let mut a = [0.0f32; TRAIT_COUNT];
        for t in &mut a {
            *t = rng.next_unit();
        }
        Genome::from_array(a)
    }

    /// A mutated copy: each trait nudged by `U[-rate, rate]`, then clamped.
    pub fn mutate(&self, rng: &mut Rng, rate: f32) -> Self {
        let mut a = self.to_array();
        for t in &mut a {
            *t += rng.next_range(-rate, rate);
        }
        Genome::from_array(a).clamped()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn round_trips_through_array() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.0, 0.0]);
        assert_eq!(g.to_array(), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.0, 0.0]);
        assert_eq!(g.diet, 0.4);
        assert_eq!(g.heat_tolerance, 0.7);
        assert_eq!(g.drought_tolerance, 0.8);
    }

    #[test]
    fn new_energy_genes_round_trip_at_the_tail() {
        let g = Genome::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.5, 0.25, 0.75]);
        assert_eq!(g.valaar_reliance, 0.5);
        assert_eq!(g.heat_affinity, 0.25);
        assert_eq!(g.water_affinity, 0.75);
        assert_eq!(TRAIT_COUNT, 12);
        assert_eq!(g.to_array()[9], 0.5);
        assert_eq!(g.to_array()[11], 0.75);
    }

    #[test]
    fn clamped_pins_to_unit_interval() {
        let g = Genome::from_array([-1.0, 2.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 2.0, 1.0, 0.0, 0.0]).clamped();
        assert_eq!(g.size, 0.0);
        assert_eq!(g.valaar_efficiency, 1.0);
        assert_eq!(g.speed, 0.5);
        assert_eq!(g.swim, 1.0);
    }

    #[test]
    fn random_is_seed_deterministic_and_in_bounds() {
        let a = Genome::random(&mut Rng::new(123));
        let b = Genome::random(&mut Rng::new(123));
        assert_eq!(a, b);
        for t in a.to_array() {
            assert!((0.0..=1.0).contains(&t), "out of range: {t}");
        }
    }

    #[test]
    fn mutate_stays_in_bounds_even_from_extremes() {
        let mut r = Rng::new(5);
        let g = Genome::from_array([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        for _ in 0..200 {
            let m = g.mutate(&mut r, 0.1);
            for t in m.to_array() {
                assert!((0.0..=1.0).contains(&t), "out of range: {t}");
            }
        }
    }

    #[test]
    fn mutate_with_zero_rate_is_identity() {
        let mut r = Rng::new(5);
        let g = Genome::from_array([0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.2, 0.9, 0.5, 1.0, 0.0, 0.0]);
        assert_eq!(g.mutate(&mut r, 0.0), g);
    }
}
