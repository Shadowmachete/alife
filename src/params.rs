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
}

impl Default for EcoParams {
    fn default() -> Self {
        EcoParams {
            base_energy: 1.0,
            size_energy: 4.0,
            basal_cost: 0.02,
            size_cost: 0.05,
            uptake_rate: 0.5,
            move_cost: 0.05,
            detritus_fraction: 0.5,
            repro_cost_fraction: 0.4,
            mutation_rate: 0.05,
            min_lifespan: 20,
            max_lifespan: 200,
            predation_efficiency: 0.6,
            heat_stress: 0.1,
            drought_stress: 0.1,
            initial_energy: 2.0,
        }
    }
}
