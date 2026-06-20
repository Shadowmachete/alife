//! Every ecology rate in one struct so `organism` and `ecology` share it
//! without a module cycle. All values are dials (edge-of-chaos tuning is later).

/// Tunable rates for the ecology loop. Defaults are *runnable placeholders*,
/// not a tuned living regime.
#[derive(Clone, Copy, Debug)]
pub struct EcoParams {
    /// Max energy storage of a size-0 organism.
    pub base_energy: f32,
    /// Extra max storage per unit `size`.
    pub size_energy: f32,
    /// Baseline energy spent per tick by a size-0 organism.
    pub basal_cost: f32,
    /// Extra basal cost per unit `size`.
    pub size_cost: f32,
    /// Fraction of a cell's valaar an ideal autotroph captures per tick.
    pub uptake_rate: f32,
    /// Energy spent to move one cell (scaled by `speed`).
    pub move_cost: f32,
    /// Fraction of a dead organism's energy returned to its cell.
    pub detritus_fraction: f32,
    /// Fraction of parent energy handed to a new child.
    pub repro_cost_fraction: f32,
    /// Per-trait mutation magnitude at birth.
    pub mutation_rate: f32,
    /// Minimum lifespan in ticks (lifespan gene 0.0).
    pub min_lifespan: u32,
    /// Maximum lifespan in ticks (lifespan gene 1.0).
    pub max_lifespan: u32,
    /// Fraction of prey energy a predator captures.
    pub predation_efficiency: f32,
    /// Energy drained per unit of heat above an organism's heat_tolerance.
    pub heat_stress: f32,
    /// Energy drained per unit of water deficit below its need.
    pub drought_stress: f32,
    /// Energy a seeded/initial organism starts with.
    pub initial_energy: f32,
    /// Energy a tunneller spends per Valaar cell crossed when it swims through.
    pub valaar_drain: f32,
    /// Mutation magnitude multiplier where local valaar is ~0 (the Dusk).
    pub mutation_floor_mult: f32,
    /// Mutation magnitude multiplier where local valaar >= `mutation_ref` (core).
    pub mutation_ceil_mult: f32,
    /// Local valaar at/above which mutation reaches `mutation_ceil_mult`.
    pub mutation_ref: f32,
    /// Extra mutation multiplier during the Rasgun surge.
    pub rasgun_mutation_mult: f32,
}

impl Default for EcoParams {
    fn default() -> Self {
        EcoParams {
            base_energy: 4.0,
            size_energy: 8.0,
            basal_cost: 0.01,
            size_cost: 0.02,
            uptake_rate: 0.4,
            move_cost: 0.03,
            detritus_fraction: 0.8,
            repro_cost_fraction: 0.4,
            mutation_rate: 0.1,
            min_lifespan: 200,
            max_lifespan: 800,
            predation_efficiency: 0.9,
            heat_stress: 0.1,
            drought_stress: 0.1,
            initial_energy: 4.0,
            valaar_drain: 0.15,
            mutation_floor_mult: 0.25,
            mutation_ceil_mult: 2.0,
            mutation_ref: 4.0,
            rasgun_mutation_mult: 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valaar_drain_default_is_present() {
        let eco = EcoParams::default();
        assert!(eco.valaar_drain > 0.0);
    }

    #[test]
    fn mutation_field_defaults_present() {
        let eco = EcoParams::default();
        assert!(eco.mutation_floor_mult < eco.mutation_ceil_mult);
        assert!(eco.mutation_ref > 0.0);
        assert!(eco.rasgun_mutation_mult > 1.0);
    }
}
