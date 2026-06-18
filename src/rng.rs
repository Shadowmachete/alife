//! A tiny seeded PRNG (SplitMix64). Hand-rolled so the whole sim stays
//! deterministic and std-only — identical seed ⇒ identical stream.

/// Deterministic pseudo-random generator. Not cryptographic.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// SplitMix64: advance state, then bit-mix to an output word.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform `f32` in `[0.0, 1.0)` using the top 24 bits (mantissa width).
    pub fn next_unit(&mut self) -> f32 {
        let bits = self.next_u64() >> 40; // keep 24 bits
        bits as f32 / (1u64 << 24) as f32
    }

    /// Uniform `f32` in `[lo, hi)`.
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        // Extremely unlikely to collide on the first draw for SplitMix64.
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn next_unit_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let u = r.next_unit();
            assert!((0.0..1.0).contains(&u), "out of range: {u}");
        }
    }

    #[test]
    fn next_range_respects_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..1000 {
            let v = r.next_range(-2.0, 5.0);
            assert!((-2.0..5.0).contains(&v), "out of range: {v}");
        }
    }
}
