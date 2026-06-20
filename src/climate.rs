//! Climate: the seasons drive per-cell heat and water. Each season has target
//! levels the fields relax toward; the genome's `heat_tolerance` /
//! `drought_tolerance` decide who can stand them (see `ecology::environmental_stress`).
//!
//! v1 is spatially uniform (one target for the whole map) — enough to make each
//! season select a different adaptation. Spatial climate is future work.

use crate::field::Field;
use crate::season::Season;

/// Where a season pushes the environment. `heat`/`water` in `[0, 1]`;
/// `valaar_mult` scales the Rasconne source for the season.
#[derive(Clone, Copy, Debug)]
pub struct SeasonTarget {
    pub heat: f32,
    pub water: f32,
    pub valaar_mult: f32,
}

/// The per-season climate table plus how fast fields chase their target.
#[derive(Clone, Debug)]
pub struct Climate {
    /// Indexed by `Season::index`.
    pub targets: [SeasonTarget; 6],
    /// Fraction of the gap closed per tick (`< 1`).
    pub relax_rate: f32,
}

impl Default for Climate {
    fn default() -> Self {
        // [ASSUMPTION A5] Wet peak = Miscre, dry peak = Dansch (lore). Heat curve
        // and valaar multipliers are an engineering guess — tune freely.
        // Order: Rasgun, Goscon, Miscre, Vraze, Dansch, Laisp.
        Climate {
            targets: [
                SeasonTarget {
                    heat: 0.30,
                    water: 0.60,
                    valaar_mult: 1.5,
                }, // Rasgun: surge, spring
                SeasonTarget {
                    heat: 0.50,
                    water: 0.60,
                    valaar_mult: 1.2,
                }, // Goscon: stable, peak
                SeasonTarget {
                    heat: 0.20,
                    water: 0.90,
                    valaar_mult: 1.0,
                }, // Miscre: monsoon, fog
                SeasonTarget {
                    heat: 0.60,
                    water: 0.40,
                    valaar_mult: 0.9,
                }, // Vraze: quakes, crystalline
                SeasonTarget {
                    heat: 0.80,
                    water: 0.15,
                    valaar_mult: 0.6,
                }, // Dansch: drought, hot
                SeasonTarget {
                    heat: 0.60,
                    water: 0.30,
                    valaar_mult: 0.4,
                }, // Laisp: lowest valaar
            ],
            relax_rate: 0.05,
        }
    }
}

/// The target for a given season.
pub fn target(climate: &Climate, season: Season) -> SeasonTarget {
    climate.targets[season.index()]
}

/// Ease every cell of `field` a `rate` fraction toward `target`.
pub fn relax_field(field: &mut Field, target: f32, rate: f32) {
    for i in 0..field.len() {
        let v = field.get(i);
        field.set(i, v + rate * (target - v));
    }
}

/// Relax both climate fields toward the current season's targets.
pub fn apply_climate(heat: &mut Field, water: &mut Field, season: Season, climate: &Climate) {
    let t = target(climate, season);
    relax_field(heat, t.heat, climate.relax_rate);
    relax_field(water, t.water, climate.relax_rate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Field;
    use crate::season::Season;

    #[test]
    fn dansch_is_the_drought_and_miscre_the_monsoon() {
        let c = Climate::default();
        let dansch = target(&c, Season::Dansch);
        let miscre = target(&c, Season::Miscre);
        assert!(
            dansch.water < miscre.water,
            "Dansch should be drier than Miscre"
        );
        assert!(dansch.water < 0.3, "Dansch is a drought");
        assert!(miscre.water > 0.7, "Miscre is a monsoon");
    }

    #[test]
    fn relax_moves_a_field_toward_target() {
        let mut f = Field::zeros(4);
        for _ in 0..200 {
            relax_field(&mut f, 0.8, 0.05);
        }
        for i in 0..f.len() {
            assert!((f.get(i) - 0.8).abs() < 1e-3, "cell {i} = {}", f.get(i));
        }
    }

    #[test]
    fn apply_climate_pushes_both_fields_to_the_season() {
        let climate = Climate::default();
        let mut heat = Field::zeros(4);
        let mut water = Field::zeros(4);
        let t = target(&climate, Season::Dansch);
        for _ in 0..300 {
            apply_climate(&mut heat, &mut water, Season::Dansch, &climate);
        }
        assert!((heat.get(0) - t.heat).abs() < 1e-2);
        assert!((water.get(0) - t.water).abs() < 1e-2);
    }
}
