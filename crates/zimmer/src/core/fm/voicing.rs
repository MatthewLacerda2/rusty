//! which of the four operators a note actually renders, and what its carriers are divided by.

use crate::patch::{Algorithm, Operator, FM_OPERATORS};

pub(crate) fn sounding(operators: &[Operator; FM_OPERATORS], ceiling: f32) -> [bool; FM_OPERATORS] {
    std::array::from_fn(|i| operators[i].ratio > 0.0 && operators[i].ratio < ceiling)
}

pub(crate) fn normaliser(
    algorithm: Algorithm,
    levels: &[f32; FM_OPERATORS],
    sounding: &[bool; FM_OPERATORS],
) -> f32 {
    let total: f32 = (0..FM_OPERATORS)
        .filter(|op| sounding[*op] && algorithm.is_carrier(*op))
        .map(|op| levels[op].max(0.0))
        .sum();
    if total > 0.0 {
        1.0 / total
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(ratio: f32) -> Operator {
        Operator {
            ratio,
            level: 1.0,
            feedback: 0.0,
            env: None,
        }
    }

    fn sounds(ratio: f32, ceiling: f32) -> bool {
        sounding(&[op(ratio), op(1.0), op(1.0), op(1.0)], ceiling)[0]
    }

    #[test]
    fn an_operator_at_or_below_dc_names_no_frequency() {
        assert!(!sounds(0.0, 4.0), "zero is a DC offset");
        assert!(
            !sounds(-1.0, 4.0),
            "and below it is not the other direction"
        );
        assert!(!sounds(f32::NAN, 4.0));
        assert!(sounds(f32::MIN_POSITIVE, 4.0), "but anything above it is");
    }

    #[test]
    fn an_operator_exactly_on_nyquist_is_dropped_like_one_above_it() {
        assert!(!sounds(4.0, 4.0), "exactly on the line");
        assert!(!sounds(4.5, 4.0), "and past it");
        assert!(sounds(3.999, 4.0), "just under it still sounds");
    }

    #[test]
    fn nothing_sounds_against_a_ceiling_of_zero() {
        for ratio in [0.0, 1.0, 1e9, f32::MAX] {
            assert!(!sounds(ratio, 0.0), "{ratio}");
        }
    }

    #[test]
    fn only_the_carriers_that_sound_are_divided_by() {
        let all = [true; FM_OPERATORS];
        let levels = [3.0, 1.0, 5.0, 3.0];
        assert_eq!(normaliser(Algorithm::Twin, &levels, &all), 0.25);
        assert_eq!(normaliser(Algorithm::Chain, &levels, &all), 1.0 / 3.0);
        let without = [true, true, true, false];
        assert_eq!(normaliser(Algorithm::Twin, &levels, &without), 1.0);
        // A modulator's level never joins the total, dropped or not.
        let quiet = [true, true, false, true];
        assert_eq!(normaliser(Algorithm::Twin, &levels, &quiet), 0.25);
    }

    #[test]
    fn carriers_at_no_level_at_all_divide_by_nothing() {
        let all = [true; FM_OPERATORS];
        let none = [false; FM_OPERATORS];
        assert_eq!(
            normaliser(Algorithm::Chain, &[3.0, 3.0, 3.0, 0.0], &all),
            0.0
        );
        assert_eq!(
            normaliser(Algorithm::Twin, &[1.0, -2.0, 1.0, -5.0], &all),
            0.0
        );
        assert_eq!(
            normaliser(Algorithm::Chain, &[3.0; FM_OPERATORS], &none),
            0.0,
            "nor when the only carrier was dropped"
        );
    }
}
