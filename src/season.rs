//! The 6-arh year. A deterministic clock: `Calendar` counts craws (days),
//! groups them into arhs (months), and reports the current `Season`. Seasons
//! drive the climate (see `climate.rs`).

/// Craws (days) per arh (month).
pub const CRAWS_PER_ARH: u32 = 117;
/// Arhs (months) per year.
pub const ARHS_PER_YEAR: u32 = 6;
/// Craws per full year.
pub const CRAWS_PER_YEAR: u32 = CRAWS_PER_ARH * ARHS_PER_YEAR;

/// The six arhs, in order. Each rewards a different adaptation.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Season {
    Rasgun,
    Goscon,
    Miscre,
    Vraze,
    Dansch,
    Laisp,
}

impl Season {
    pub const ALL: [Season; 6] = [
        Season::Rasgun,
        Season::Goscon,
        Season::Miscre,
        Season::Vraze,
        Season::Dansch,
        Season::Laisp,
    ];

    pub fn index(self) -> usize {
        match self {
            Season::Rasgun => 0,
            Season::Goscon => 1,
            Season::Miscre => 2,
            Season::Vraze => 3,
            Season::Dansch => 4,
            Season::Laisp => 5,
        }
    }
}

/// Deterministic day counter. `craw` is the day within the current year.
#[derive(Clone, Debug, Default)]
pub struct Calendar {
    craw: u32,
    year: u32,
}

impl Calendar {
    pub fn new() -> Self {
        Calendar { craw: 0, year: 0 }
    }

    /// Advance one craw (one tick), wrapping the year.
    pub fn advance(&mut self) {
        self.craw += 1;
        if self.craw >= CRAWS_PER_YEAR {
            self.craw = 0;
            self.year += 1;
        }
    }

    pub fn craw(&self) -> u32 {
        self.craw
    }

    pub fn arh(&self) -> u32 {
        self.craw / CRAWS_PER_ARH
    }

    pub fn year(&self) -> u32 {
        self.year
    }

    pub fn season(&self) -> Season {
        Season::ALL[self.arh() as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_has_six_arhs_of_117_craws() {
        assert_eq!(CRAWS_PER_ARH, 117);
        assert_eq!(ARHS_PER_YEAR, 6);
        assert_eq!(CRAWS_PER_YEAR, 702);
        assert_eq!(Season::ALL.len(), 6);
    }

    #[test]
    fn calendar_starts_at_rasgun() {
        let c = Calendar::new();
        assert_eq!(c.craw(), 0);
        assert_eq!(c.arh(), 0);
        assert_eq!(c.year(), 0);
        assert_eq!(c.season(), Season::Rasgun);
    }

    #[test]
    fn arh_boundary_advances_the_season() {
        let mut c = Calendar::new();
        for _ in 0..CRAWS_PER_ARH {
            c.advance();
        }
        assert_eq!(c.arh(), 1);
        assert_eq!(c.season(), Season::Goscon);
        assert_eq!(c.craw(), CRAWS_PER_ARH);
    }

    #[test]
    fn year_wraps_after_all_six_arhs() {
        let mut c = Calendar::new();
        for _ in 0..CRAWS_PER_YEAR {
            c.advance();
        }
        assert_eq!(c.year(), 1);
        assert_eq!(c.craw(), 0);
        assert_eq!(c.season(), Season::Rasgun);
    }

    #[test]
    fn seasons_follow_the_lore_order() {
        let order = [
            Season::Rasgun,
            Season::Goscon,
            Season::Miscre,
            Season::Vraze,
            Season::Dansch,
            Season::Laisp,
        ];
        let mut c = Calendar::new();
        for (arh, &want) in order.iter().enumerate() {
            // jump to the middle of arh `arh`
            let target = arh as u32 * CRAWS_PER_ARH + 10;
            while c.craw() < target {
                c.advance();
            }
            assert_eq!(c.season(), want, "arh {arh}");
        }
    }
}
