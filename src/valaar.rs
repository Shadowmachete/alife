//! Valaar dynamics — pure functions over `(Space, Field)`, composed by
//! `World::step`. Valaar is the world's single energy currency.

use crate::field::Field;
use crate::season::Season;
use crate::space::{Coord, Layer, Space};

/// The physical *phase* valaar takes in a given season. Beyond abundance (the
/// climate `valaar_mult` on the source), the phase changes valaar's *dynamics*:
/// how far it spreads, how fast it drains, and whether it crystallises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValaarPhase {
    /// Rasgun / Goscon — flows and diffuses normally.
    Liquid,
    /// Miscre — gaseous/fog: spreads much further (airborne valaar reaches the Dusk).
    Gaseous,
    /// Vraze — crystalline: stops spreading and freezes a fraction of valaar into
    /// solid `crystal` that resists decay until later seasons thaw it.
    Crystalline,
    /// Dansch / Laisp — sparse: valaar drains faster and the Dusk bites hardest.
    Sparse,
}

/// Per-phase dynamics dials.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseDynamics {
    /// Diffusion passes per tick (0 = locked). Multiple *stable* passes spread
    /// valaar further without exceeding the per-pass coefficient limit (< 0.25).
    pub diffuse_passes: u32,
    /// Multiplier on the base decay rate this phase.
    pub decay_mult: f32,
    /// Fraction of each cell's valaar that freezes into crystal per tick.
    pub freeze_rate: f32,
    /// Fraction of each cell's crystal that thaws back into valaar per tick.
    pub thaw_rate: f32,
}

impl ValaarPhase {
    /// The phase valaar takes in `season`.
    pub fn for_season(season: Season) -> Self {
        match season {
            Season::Rasgun | Season::Goscon => ValaarPhase::Liquid,
            Season::Miscre => ValaarPhase::Gaseous,
            Season::Vraze => ValaarPhase::Crystalline,
            Season::Dansch | Season::Laisp => ValaarPhase::Sparse,
        }
    }

    /// The tuning dials for this phase (tune the whole state-machine here).
    pub fn dynamics(self) -> PhaseDynamics {
        match self {
            ValaarPhase::Liquid => PhaseDynamics {
                diffuse_passes: 1,
                decay_mult: 1.0,
                freeze_rate: 0.0,
                thaw_rate: 0.02,
            },
            ValaarPhase::Gaseous => PhaseDynamics {
                diffuse_passes: 3,
                decay_mult: 1.0,
                freeze_rate: 0.0,
                thaw_rate: 0.02,
            },
            ValaarPhase::Crystalline => PhaseDynamics {
                diffuse_passes: 0,
                decay_mult: 0.5,
                freeze_rate: 0.20,
                thaw_rate: 0.0,
            },
            ValaarPhase::Sparse => PhaseDynamics {
                diffuse_passes: 1,
                decay_mult: 1.5,
                freeze_rate: 0.0,
                thaw_rate: 0.02,
            },
        }
    }
}

/// Move valaar between the liquid field and the frozen `crystal` field. During
/// the `Crystalline` phase a `freeze_rate` fraction of each cell's valaar freezes
/// into crystal; in every other phase a `thaw_rate` fraction of crystal thaws
/// back into valaar. Crystal never diffuses or decays, so it carries valaar
/// forward in time. Conserves the `valaar + crystal` total.
pub fn freeze_thaw(valaar: &mut Field, crystal: &mut Field, phase: ValaarPhase, d: &PhaseDynamics) {
    match phase {
        ValaarPhase::Crystalline => {
            for i in 0..valaar.len() {
                let f = valaar.get(i) * d.freeze_rate;
                valaar.set(i, valaar.get(i) - f);
                crystal.add(i, f);
            }
        }
        _ => {
            for i in 0..crystal.len() {
                let t = crystal.get(i) * d.thaw_rate;
                crystal.set(i, crystal.get(i) - t);
                valaar.add(i, t);
            }
        }
    }
}

/// Inject valaar at each source cell (e.g. the Rasconne reservoir).
pub fn inject_sources<S: Space>(space: &S, field: &mut Field, sources: &[Coord], rate: f32) {
    for &c in sources {
        field.add(space.index(c), rate);
    }
}

/// Uniform exponential decay: every cell loses `decay` fraction this step.
pub fn decay(field: &mut Field, decay: f32) {
    field.scale_all(1.0 - decay);
}

/// Explicit planar diffusion on each layer (discrete Laplacian). Conserves total
/// valaar exactly (no-flux boundaries) because every neighbour exchange is
/// counted symmetrically. Keep `rate < 0.25` for stability.
pub fn diffuse_planar<S: Space>(space: &S, field: &mut Field, rate: f32) {
    let mut delta = vec![0.0f32; field.len()];
    for layer in Layer::ALL {
        for y in 0..space.height() {
            for x in 0..space.width() {
                let c = Coord::new(x, y, layer);
                let i = space.index(c);
                let here = field.get(i);
                for n in space.planar_neighbors(c) {
                    let j = space.index(n);
                    delta[i] += rate * (field.get(j) - here);
                }
            }
        }
    }
    for (i, d) in delta.iter().enumerate() {
        field.add(i, *d);
    }
}

/// Exchange valaar between surface and underground at access points only.
/// Conserves total valaar (flux out of one layer equals flux into the other).
pub fn exchange_layers<S: Space>(
    space: &S,
    field: &mut Field,
    access_points: &[(u32, u32)],
    rate: f32,
) {
    for &(x, y) in access_points {
        let s = space.index(Coord::new(x, y, Layer::Surface));
        let u = space.index(Coord::new(x, y, Layer::Underground));
        let flux = rate * (field.get(u) - field.get(s));
        field.add(s, flux);
        field.add(u, -flux);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Field;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    #[test]
    fn inject_adds_rate_at_each_source() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        let sources = [
            Coord::new(1, 1, Layer::Surface),
            Coord::new(2, 2, Layer::Surface),
        ];
        inject_sources(&space, &mut field, &sources, 3.0);
        assert_eq!(
            field.get(space.index(Coord::new(1, 1, Layer::Surface))),
            3.0
        );
        assert_eq!(
            field.get(space.index(Coord::new(2, 2, Layer::Surface))),
            3.0
        );
        assert_eq!(field.total(), 6.0);
    }

    #[test]
    fn decay_scales_total_down() {
        let space = Grid2p5D::new(2, 2);
        let mut field = Field::zeros(space.len());
        field.set(space.index(Coord::new(0, 0, Layer::Surface)), 10.0);
        decay(&mut field, 0.1);
        assert!((field.total() - 9.0).abs() < 1e-6);
    }

    #[test]
    fn diffusion_conserves_total() {
        let space = Grid2p5D::new(8, 8);
        let mut field = Field::zeros(space.len());
        field.set(space.index(Coord::new(4, 4, Layer::Surface)), 100.0);
        let before = field.total();
        for _ in 0..20 {
            diffuse_planar(&space, &mut field, 0.2);
        }
        assert!(
            (field.total() - before).abs() < 1e-3,
            "total drifted: {}",
            field.total()
        );
    }

    #[test]
    fn diffusion_spreads_a_spike_to_neighbors() {
        let space = Grid2p5D::new(8, 8);
        let mut field = Field::zeros(space.len());
        let center = Coord::new(4, 4, Layer::Surface);
        field.set(space.index(center), 100.0);
        diffuse_planar(&space, &mut field, 0.2);
        let neighbor = Coord::new(5, 4, Layer::Surface);
        assert!(field.get(space.index(center)) < 100.0, "spike should drop");
        assert!(
            field.get(space.index(neighbor)) > 0.0,
            "neighbour should rise"
        );
    }

    #[test]
    fn diffusion_does_not_cross_layers() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        field.set(space.index(Coord::new(2, 2, Layer::Surface)), 50.0);
        diffuse_planar(&space, &mut field, 0.2);
        // underground stays empty: planar diffusion never crosses layers
        assert_eq!(
            field.get(space.index(Coord::new(2, 2, Layer::Underground))),
            0.0
        );
    }

    #[test]
    fn exchange_moves_valaar_between_layers_and_conserves() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        let surf = space.index(Coord::new(2, 2, Layer::Surface));
        let under = space.index(Coord::new(2, 2, Layer::Underground));
        field.set(under, 10.0); // underground reservoir
        let before = field.total();
        exchange_layers(&space, &mut field, &[(2, 2)], 0.1);
        assert!(field.get(surf) > 0.0, "surface should gain from below");
        assert!(field.get(under) < 10.0, "underground should drop");
        assert!(
            (field.total() - before).abs() < 1e-6,
            "exchange must conserve"
        );
    }

    #[test]
    fn exchange_only_at_access_points() {
        let space = Grid2p5D::new(4, 4);
        let mut field = Field::zeros(space.len());
        let under = space.index(Coord::new(0, 0, Layer::Underground));
        let surf = space.index(Coord::new(0, 0, Layer::Surface));
        field.set(under, 10.0);
        exchange_layers(&space, &mut field, &[(2, 2)], 0.1); // (0,0) is NOT an access point
        assert_eq!(field.get(surf), 0.0);
        assert_eq!(field.get(under), 10.0);
    }

    #[test]
    fn phase_maps_each_season() {
        use ValaarPhase::*;
        assert_eq!(ValaarPhase::for_season(Season::Rasgun), Liquid);
        assert_eq!(ValaarPhase::for_season(Season::Goscon), Liquid);
        assert_eq!(ValaarPhase::for_season(Season::Miscre), Gaseous);
        assert_eq!(ValaarPhase::for_season(Season::Vraze), Crystalline);
        assert_eq!(ValaarPhase::for_season(Season::Dansch), Sparse);
        assert_eq!(ValaarPhase::for_season(Season::Laisp), Sparse);
    }

    #[test]
    fn phase_dynamics_match_their_intent() {
        let c = ValaarPhase::Crystalline.dynamics();
        assert_eq!(c.diffuse_passes, 0, "crystalline locks valaar in place");
        assert!(
            c.freeze_rate > 0.0 && c.thaw_rate == 0.0,
            "crystalline freezes, never thaws"
        );
        let g = ValaarPhase::Gaseous.dynamics();
        assert!(
            g.diffuse_passes > ValaarPhase::Liquid.dynamics().diffuse_passes,
            "gaseous spreads further than liquid"
        );
        let s = ValaarPhase::Sparse.dynamics();
        assert!(s.decay_mult > 1.0, "sparse drains faster");
        assert!(
            ValaarPhase::Liquid.dynamics().thaw_rate > 0.0,
            "non-crystalline thaws crystal back"
        );
    }

    #[test]
    fn freeze_moves_valaar_into_crystal_and_conserves() {
        let mut v = Field::zeros(2);
        let mut c = Field::zeros(2);
        v.set(0, 1.0);
        let d = ValaarPhase::Crystalline.dynamics(); // freeze_rate 0.10
        let before = v.total() + c.total();
        freeze_thaw(&mut v, &mut c, ValaarPhase::Crystalline, &d);
        assert!((c.get(0) - 0.10).abs() < 1e-6);
        assert!((v.get(0) - 0.90).abs() < 1e-6);
        assert!(
            (v.total() + c.total() - before).abs() < 1e-6,
            "valaar+crystal conserved"
        );
    }

    #[test]
    fn thaw_returns_crystal_to_valaar_off_crystalline() {
        let mut v = Field::zeros(1);
        let mut c = Field::zeros(1);
        c.set(0, 1.0);
        let d = ValaarPhase::Liquid.dynamics(); // thaw_rate 0.02
        freeze_thaw(&mut v, &mut c, ValaarPhase::Liquid, &d);
        assert!((c.get(0) - 0.98).abs() < 1e-6);
        assert!((v.get(0) - 0.02).abs() < 1e-6);
    }
}
