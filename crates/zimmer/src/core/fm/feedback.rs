//! Bounded: the fed-back value is an operator's output (±1), any amount but `NaN`
//! clamps to `0..=1`, and it only offsets a phase, so no state can grow.

use std::f32::consts::PI;

/// Radians: half a cycle, the ceiling that makes `Operator::feedback` a `0..=1` knob.
pub(crate) const MAX_FEEDBACK: f32 = PI;

pub(crate) fn bend(amount: f32, last: [f32; 2]) -> f32 {
    amount.clamp(0.0, 1.0) * MAX_FEEDBACK * (last[0] + last[1]) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bend_is_bounded_however_the_amount_is_written() {
        for amount in [1.0, 12.0, 1e9, f32::MAX, f32::INFINITY] {
            assert_eq!(bend(amount, [1.0, 1.0]), MAX_FEEDBACK, "{amount}");
            assert_eq!(bend(amount, [-1.0, -1.0]), -MAX_FEEDBACK, "{amount}");
        }
        for amount in [0.0, -1.0, -1e9, f32::NEG_INFINITY] {
            assert_eq!(bend(amount, [1.0, 1.0]), 0.0, "{amount}");
        }
    }

    #[test]
    fn the_two_stored_samples_are_averaged() {
        assert_eq!(bend(1.0, [1.0, -1.0]), 0.0);
        assert_eq!(bend(1.0, [1.0, 0.0]), MAX_FEEDBACK * 0.5);
        assert_eq!(bend(1.0, [0.0, 1.0]), MAX_FEEDBACK * 0.5);
        assert_eq!(bend(0.5, [1.0, 1.0]), MAX_FEEDBACK * 0.5);
    }
}
