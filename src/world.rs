//! The simulated world: a `Space`, the valaar `Field`, and the sources /
//! access points that shape valaar flow.

use crate::field::Field;
use crate::space::{Coord, Space};

/// Tunable rates for valaar dynamics. Every rate is a dial (edge-of-chaos).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Valaar injected at each source cell per step.
    pub source_rate: f32,
    /// Planar diffusion coefficient. Keep `< 0.25` for stability.
    pub diffuse_rate: f32,
    /// Surface<->underground exchange fraction at access points.
    pub layer_exchange: f32,
    /// Fraction of valaar lost per step, in `0.0..=1.0`.
    pub decay: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            source_rate: 1.0,
            diffuse_rate: 0.2,
            layer_exchange: 0.1,
            decay: 0.01,
        }
    }
}

pub struct World<S: Space> {
    pub space: S,
    pub valaar: Field,
    pub params: Params,
    sources: Vec<Coord>,
    access_points: Vec<(u32, u32)>,
    /// Per-cell passability (sized to `space.len()`); `None` = everywhere
    /// passable. Set from the terrain map; consumed by `ecology::move_organisms`.
    passability: Option<Vec<bool>>,
}

impl<S: Space> World<S> {
    pub fn new(space: S, params: Params) -> Self {
        let valaar = Field::zeros(space.len());
        World {
            space,
            valaar,
            params,
            sources: Vec::new(),
            access_points: Vec::new(),
            passability: None,
        }
    }

    /// Register a valaar source cell (e.g. the Rasconne reservoir).
    pub fn add_source(&mut self, c: Coord) {
        debug_assert!(self.space.in_bounds(c));
        self.sources.push(c);
    }

    /// Register an `(x, y)` column where surface and underground exchange valaar.
    pub fn add_access_point(&mut self, x: u32, y: u32) {
        self.access_points.push((x, y));
    }

    pub fn sources(&self) -> &[Coord] {
        &self.sources
    }

    pub fn access_points(&self) -> &[(u32, u32)] {
        &self.access_points
    }

    /// Install a per-cell passability mask (`true` = passable). Length must
    /// equal `space.len()` (all layers, `Space::index` order).
    pub fn set_passability(&mut self, mask: Vec<bool>) {
        debug_assert_eq!(mask.len(), self.space.len(), "mask must cover every cell");
        self.passability = Some(mask);
    }

    /// The passability mask, if one was installed.
    pub fn passability(&self) -> Option<&[bool]> {
        self.passability.as_deref()
    }
}

impl<S: Space> World<S> {
    /// Advance one tick: inject sources, diffuse, exchange between layers, decay.
    pub fn step(&mut self) {
        use crate::valaar;
        valaar::inject_sources(&self.space, &mut self.valaar, &self.sources, self.params.source_rate);
        valaar::diffuse_planar(&self.space, &mut self.valaar, self.params.diffuse_rate);
        valaar::exchange_layers(
            &self.space,
            &mut self.valaar,
            &self.access_points,
            self.params.layer_exchange,
        );
        valaar::decay(&mut self.valaar, self.params.decay);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    #[test]
    fn new_world_has_zeroed_field_sized_to_space() {
        let space = Grid2p5D::new(4, 4);
        let world = World::new(space, Params::default());
        assert_eq!(world.valaar.len(), world.space.len());
        assert_eq!(world.valaar.total(), 0.0);
        assert!(world.sources().is_empty());
        assert!(world.access_points().is_empty());
    }

    #[test]
    fn sources_and_access_points_record() {
        let space = Grid2p5D::new(4, 4);
        let mut world = World::new(space, Params::default());
        world.add_source(Coord::new(2, 2, Layer::Surface));
        world.add_access_point(2, 2);
        assert_eq!(world.sources(), &[Coord::new(2, 2, Layer::Surface)]);
        assert_eq!(world.access_points(), &[(2, 2)]);
    }

    #[test]
    fn passability_defaults_none_and_round_trips() {
        let space = Grid2p5D::new(2, 2);
        let mut world = World::new(space, Params::default());
        assert!(world.passability().is_none());
        let mask = vec![true; world.space.len()];
        world.set_passability(mask);
        assert_eq!(world.passability().unwrap().len(), world.space.len());
        assert!(world.passability().unwrap().iter().all(|&p| p));
    }
}
