//! Spatial model for the world.
//!
//! INVARIANT: the world is **2-dimensional** (a planar grid). The vertical axis
//! is a *discrete* `Layer` (Surface / Underground), NOT a continuous third
//! dimension ("2.5D"). All code outside this module MUST talk to space through
//! the `Space` trait so the dimensionality stays swappable.
//!
//! FUTURE (3D): to make the world truly 3D, change `SPATIAL_DIMS` to 3 and add a
//! `Grid3D` implementation of `Space`. Nothing outside this module should need to
//! change.

/// Number of *continuous* spatial dimensions. The world is planar (2D); the
/// surface/underground split is a discrete layer, not a third axis.
pub const SPATIAL_DIMS: usize = 2;

/// How many discrete vertical layers exist.
pub const LAYER_COUNT: usize = 2;

/// Discrete vertical layers (the "2.5D" of the model).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Layer {
    Surface = 0,
    Underground = 1,
}

impl Layer {
    /// All layers, in index order.
    pub const ALL: [Layer; LAYER_COUNT] = [Layer::Surface, Layer::Underground];
}

/// A cell coordinate: planar `(x, y)` plus a discrete `layer`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Coord {
    pub x: u32,
    pub y: u32,
    pub layer: Layer,
}

impl Coord {
    pub fn new(x: u32, y: u32, layer: Layer) -> Self {
        Coord { x, y, layer }
    }
}

/// The spatial topology the rest of the engine talks to. Implementations decide
/// dimensionality and indexing; all other code is dimension-agnostic.
pub trait Space {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    /// Total cells across all layers — the length of a per-cell array.
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn in_bounds(&self, c: Coord) -> bool;
    /// Flat array index for a cell. Caller ensures `in_bounds`.
    fn index(&self, c: Coord) -> usize;
    /// Same-layer 4-neighbours that are in bounds.
    fn planar_neighbors(&self, c: Coord) -> Vec<Coord>;
}

/// A planar grid with discrete Surface/Underground layers ("2.5D").
#[derive(Clone, Debug)]
pub struct Grid2p5D {
    width: u32,
    height: u32,
}

impl Grid2p5D {
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width > 0 && height > 0, "grid must be non-empty");
        Grid2p5D { width, height }
    }

    fn plane_len(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

impl Space for Grid2p5D {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn len(&self) -> usize {
        LAYER_COUNT * self.plane_len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn in_bounds(&self, c: Coord) -> bool {
        c.x < self.width && c.y < self.height
    }

    fn index(&self, c: Coord) -> usize {
        debug_assert!(self.in_bounds(c));
        (c.layer as usize) * self.plane_len() + c.y as usize * self.width as usize + c.x as usize
    }

    fn planar_neighbors(&self, c: Coord) -> Vec<Coord> {
        let mut out = Vec::with_capacity(4);
        if c.x > 0 {
            out.push(Coord::new(c.x - 1, c.y, c.layer));
        }
        if c.x + 1 < self.width {
            out.push(Coord::new(c.x + 1, c.y, c.layer));
        }
        if c.y > 0 {
            out.push(Coord::new(c.x, c.y - 1, c.layer));
        }
        if c.y + 1 < self.height {
            out.push(Coord::new(c.x, c.y + 1, c.layer));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_is_two_dimensional() {
        assert_eq!(SPATIAL_DIMS, 2);
        assert_eq!(LAYER_COUNT, 2);
        assert_eq!(Layer::ALL.len(), LAYER_COUNT);
        assert_eq!(Layer::Surface as usize, 0);
        assert_eq!(Layer::Underground as usize, 1);
    }

    #[test]
    fn coord_constructs() {
        let c = Coord::new(3, 4, Layer::Underground);
        assert_eq!(c.x, 3);
        assert_eq!(c.y, 4);
        assert_eq!(c.layer, Layer::Underground);
    }

    #[test]
    fn grid_len_counts_all_layers() {
        let g = Grid2p5D::new(4, 3);
        assert_eq!(g.width(), 4);
        assert_eq!(g.height(), 3);
        assert_eq!(g.len(), LAYER_COUNT * 4 * 3);
    }

    #[test]
    fn index_is_unique_per_cell() {
        let g = Grid2p5D::new(4, 3);
        let mut seen = std::collections::HashSet::new();
        for layer in Layer::ALL {
            for y in 0..g.height() {
                for x in 0..g.width() {
                    let i = g.index(Coord::new(x, y, layer));
                    assert!(i < g.len());
                    assert!(seen.insert(i), "index {i} repeated");
                }
            }
        }
        assert_eq!(seen.len(), g.len());
    }

    #[test]
    fn in_bounds_rejects_outside() {
        let g = Grid2p5D::new(4, 3);
        assert!(g.in_bounds(Coord::new(3, 2, Layer::Surface)));
        assert!(!g.in_bounds(Coord::new(4, 2, Layer::Surface)));
        assert!(!g.in_bounds(Coord::new(0, 3, Layer::Underground)));
    }

    #[test]
    fn planar_neighbors_respect_edges_and_layer() {
        let g = Grid2p5D::new(4, 3);
        // interior cell -> 4 neighbours
        assert_eq!(
            g.planar_neighbors(Coord::new(1, 1, Layer::Surface)).len(),
            4
        );
        // corner -> 2
        assert_eq!(
            g.planar_neighbors(Coord::new(0, 0, Layer::Surface)).len(),
            2
        );
        // edge -> 3
        assert_eq!(
            g.planar_neighbors(Coord::new(1, 0, Layer::Surface)).len(),
            3
        );
        // neighbours never cross layers
        for n in g.planar_neighbors(Coord::new(1, 1, Layer::Underground)) {
            assert_eq!(n.layer, Layer::Underground);
        }
    }
}
