//! A capped time-series of population snapshots for the viewer's charts. Pure
//! data: the viewer pushes a `Snapshot` each sample and reads back `egui_plot`-
//! ready `Vec<[f64; 2]>` series.

/// One continent's data point within a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinentPoint {
    pub label: u32,
    pub count: usize,
    pub mean_size: f32,
}

/// One sampled moment in time.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// X coordinate for the plot (absolute craw).
    pub x: f64,
    pub total: usize,
    pub mean_size: f32,
    pub continents: Vec<ContinentPoint>,
}

/// A capped buffer of snapshots; oldest points drop once `cap` is exceeded.
pub struct History {
    points: Vec<Snapshot>,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> Self {
        History { points: Vec::new(), cap }
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Append a snapshot, dropping the oldest if over capacity.
    pub fn push(&mut self, s: Snapshot) {
        self.points.push(s);
        if self.points.len() > self.cap {
            let overflow = self.points.len() - self.cap;
            self.points.drain(0..overflow);
        }
    }

    /// `(x, total population)` over time.
    pub fn total_series(&self) -> Vec<[f64; 2]> {
        self.points.iter().map(|s| [s.x, s.total as f64]).collect()
    }

    /// `(x, overall mean body size)` over time.
    pub fn mean_size_series(&self) -> Vec<[f64; 2]> {
        self.points.iter().map(|s| [s.x, s.mean_size as f64]).collect()
    }

    /// `(x, population)` over time for one continent label (points where the
    /// continent was present).
    pub fn continent_count_series(&self, label: u32) -> Vec<[f64; 2]> {
        self.points
            .iter()
            .filter_map(|s| s.continents.iter().find(|c| c.label == label).map(|c| [s.x, c.count as f64]))
            .collect()
    }

    /// `(x, mean body size)` over time for one continent label.
    pub fn continent_size_series(&self, label: u32) -> Vec<[f64; 2]> {
        self.points
            .iter()
            .filter_map(|s| s.continents.iter().find(|c| c.label == label).map(|c| [s.x, c.mean_size as f64]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(x: f64, total: usize) -> Snapshot {
        Snapshot {
            x,
            total,
            mean_size: 0.5,
            continents: vec![ContinentPoint { label: 0, count: total, mean_size: 0.5 }],
        }
    }

    #[test]
    fn push_caps_and_drops_oldest() {
        let mut h = History::new(3);
        for i in 0..5 {
            h.push(snap(i as f64, i));
        }
        assert_eq!(h.len(), 3);
        // oldest two (x=0,1) dropped; series starts at x=2.
        let series = h.total_series();
        assert_eq!(series.first().unwrap()[0], 2.0);
        assert_eq!(series.last().unwrap()[0], 4.0);
    }

    #[test]
    fn series_extract_x_and_value() {
        let mut h = History::new(10);
        h.push(snap(0.0, 7));
        h.push(snap(5.0, 9));
        assert_eq!(h.total_series(), vec![[0.0, 7.0], [5.0, 9.0]]);
        assert_eq!(h.continent_count_series(0), vec![[0.0, 7.0], [5.0, 9.0]]);
        assert!(h.continent_count_series(99).is_empty(), "absent continent => empty");
    }

    #[test]
    fn clear_empties() {
        let mut h = History::new(10);
        h.push(snap(0.0, 1));
        h.clear();
        assert!(h.is_empty());
    }
}
