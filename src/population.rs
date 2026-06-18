//! The organism store: a flat `Vec<TraitOrganism>` (GPU/.npy-portable later)
//! plus a per-cell occupancy index rebuilt on demand for local interactions.
//!
//! Monomorphic on `TraitOrganism` for now (the only clade). When a second clade
//! lands, generalise to `Population<O: Organism>` or per-clade stores — the
//! `Organism` trait already provides the shared accessors that would need.

use crate::organism::TraitOrganism;
use crate::space::Space;

#[derive(Default)]
pub struct Population {
    orgs: Vec<TraitOrganism>,
}

impl Population {
    pub fn new() -> Self {
        Population { orgs: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.orgs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orgs.is_empty()
    }

    pub fn spawn(&mut self, o: TraitOrganism) {
        self.orgs.push(o);
    }

    pub fn organisms(&self) -> &[TraitOrganism] {
        &self.orgs
    }

    pub fn organisms_mut(&mut self) -> &mut [TraitOrganism] {
        &mut self.orgs
    }

    /// Lists of organism indices per cell, indexed by `Space::index`. Rebuilt
    /// each call so it always matches the current positions.
    pub fn occupancy<S: Space>(&self, space: &S) -> Vec<Vec<usize>> {
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); space.len()];
        for (i, o) in self.orgs.iter().enumerate() {
            cells[space.index(o.pos)].push(i);
        }
        cells
    }

    /// Keep only organisms for which `keep` is true (preserves order).
    pub fn retain(&mut self, keep: impl Fn(&TraitOrganism) -> bool) {
        self.orgs.retain(|o| keep(o));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::organism::TraitOrganism;
    use crate::space::{Coord, Grid2p5D, Layer, Space};

    fn org_at(c: Coord) -> TraitOrganism {
        TraitOrganism::new(Genome::from_array([0.5; 6]), c, 1.0)
    }

    #[test]
    fn spawn_grows_population() {
        let mut p = Population::new();
        assert!(p.is_empty());
        p.spawn(org_at(Coord::new(0, 0, Layer::Surface)));
        p.spawn(org_at(Coord::new(1, 0, Layer::Surface)));
        assert_eq!(p.len(), 2);
        assert!(!p.is_empty());
    }

    #[test]
    fn occupancy_buckets_by_cell() {
        let space = Grid2p5D::new(4, 4);
        let mut p = Population::new();
        let c = Coord::new(2, 2, Layer::Surface);
        p.spawn(org_at(c));
        p.spawn(org_at(c)); // same cell
        p.spawn(org_at(Coord::new(0, 0, Layer::Surface)));
        let occ = p.occupancy(&space);
        assert_eq!(occ.len(), space.len());
        assert_eq!(occ[space.index(c)].len(), 2);
        assert_eq!(occ[space.index(Coord::new(0, 0, Layer::Surface))].len(), 1);
    }

    #[test]
    fn retain_drops_unwanted() {
        let mut p = Population::new();
        let mut a = org_at(Coord::new(0, 0, Layer::Surface));
        a.energy = 0.0;
        p.spawn(a);
        p.spawn(org_at(Coord::new(1, 0, Layer::Surface))); // energy 1.0
        p.retain(|o| o.energy > 0.0);
        assert_eq!(p.len(), 1);
    }
}
