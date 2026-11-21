//! Decibels and linear gain, the one conversion every level reading and every
//! gain stage needs.
//!
//! Each function is the textbook expression and nothing more, so a caller that
//! used to write it inline gets the same bits back: a bake is addressed by a
//! hash of its samples' inputs, and moving one onto these must not move a
//! sample. The `f32` pair exists because the synthesis path works in `f32` and
//! the measuring path in `f64`; converting through the other precision would
//! round differently, so each keeps its own.

/// The linear amplitude factor `db` decibels stands for: `0.0` is unity,
/// `-6.0` about half.
pub fn db_to_gain(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

/// The decibels a linear amplitude factor `gain` stands for: `1.0` is `0.0`,
/// and silence is negative infinity.
pub fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.log10()
}

/// [`db_to_gain`] in `f32`, for the synthesis path.
pub fn db_to_gain_f32(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// [`gain_to_db`] in `f32`, for the synthesis path.
pub fn gain_to_db_f32(gain: f32) -> f32 {
    20.0 * gain.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_is_zero_decibels_both_ways() {
        assert_eq!(db_to_gain(0.0), 1.0);
        assert_eq!(gain_to_db(1.0), 0.0);
        assert_eq!(db_to_gain_f32(0.0), 1.0);
        assert_eq!(gain_to_db_f32(1.0), 0.0);
    }

    #[test]
    fn twenty_decibels_is_a_factor_of_ten() {
        assert!((db_to_gain(-20.0) - 0.1).abs() < 1e-12);
        assert!((gain_to_db(10.0) - 20.0).abs() < 1e-12);
        assert!((db_to_gain_f32(20.0) - 10.0).abs() < 1e-5);
        assert!((gain_to_db_f32(0.1) + 20.0).abs() < 1e-5);
    }

    #[test]
    fn the_two_directions_undo_each_other() {
        for db in [-60.0, -6.0, 3.5, 12.0] {
            assert!((gain_to_db(db_to_gain(db)) - db).abs() < 1e-9);
            let db = db as f32;
            assert!((gain_to_db_f32(db_to_gain_f32(db)) - db).abs() < 1e-4);
        }
    }

    #[test]
    fn silence_is_negative_infinity() {
        assert_eq!(gain_to_db(0.0), f64::NEG_INFINITY);
        assert_eq!(gain_to_db_f32(0.0), f32::NEG_INFINITY);
    }
}
