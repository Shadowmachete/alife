//! A scalar field: one `f32` per cell, indexed by a `Space`'s flat index.
//! Stored as a flat `Vec<f32>` so it ports directly to a GPU buffer later.

#[derive(Clone, Debug)]
pub struct Field {
    data: Vec<f32>,
}

impl Field {
    pub fn zeros(len: usize) -> Self {
        Field { data: vec![0.0; len] }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, i: usize) -> f32 {
        self.data[i]
    }

    pub fn set(&mut self, i: usize, v: f32) {
        self.data[i] = v;
    }

    pub fn add(&mut self, i: usize, dv: f32) {
        self.data[i] += dv;
    }

    pub fn total(&self) -> f32 {
        self.data.iter().sum()
    }

    pub fn scale_all(&mut self, factor: f32) {
        for v in &mut self.data {
            *v *= factor;
        }
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_starts_empty_of_value() {
        let f = Field::zeros(5);
        assert_eq!(f.len(), 5);
        assert_eq!(f.total(), 0.0);
    }

    #[test]
    fn set_get_add_total() {
        let mut f = Field::zeros(3);
        f.set(0, 2.0);
        f.add(0, 0.5);
        f.set(2, 1.0);
        assert_eq!(f.get(0), 2.5);
        assert_eq!(f.get(1), 0.0);
        assert_eq!(f.total(), 3.5);
    }

    #[test]
    fn scale_all_scales_total() {
        let mut f = Field::zeros(2);
        f.set(0, 4.0);
        f.set(1, 6.0);
        f.scale_all(0.5);
        assert_eq!(f.total(), 5.0);
    }
}
