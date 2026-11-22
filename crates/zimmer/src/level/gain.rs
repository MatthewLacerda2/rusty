//! The textbook expressions and nothing more, so moving inline arithmetic onto them keeps every
//! sample's bits. The `f32` pair is the synthesis path's: going through `f64` rounds differently.

pub fn db_to_gain(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

pub fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.log10()
}

pub fn db_to_gain_f32(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

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
