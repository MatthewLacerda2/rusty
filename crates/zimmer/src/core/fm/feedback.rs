//! an operator bending its own phase, and why that is not a loop a recipe can
//! break.
//!
//! Self-modulation is the standard way an FM voice reaches the rasping,
//! noise-like end of its range: low down it fattens a sine toward a saw, and
//! near the top the operator breaks up into a growl. Nothing else in this
//! crate reaches that timbre — every other stage is a straight line except the
//! saturator, which colours a signal rather than changing what it is.
//!
//! It is also the one place in the whole signal path where an output is read
//! back as an input, and [`patch`](crate::patch) is explicit that a recipe must
//! never be able to write *a feedback loop with no output, a stage that never
//! terminates*. So the bound is stated here rather than left to be inferred:
//!
//! - The fed-back value is an operator's output, which is a sine's output, so
//!   it is in `−1..=1` whatever came before it.
//! - The recipe's amount is clamped to `0..=1` and multiplied by
//!   [`MAX_FEEDBACK`] radians, so the term is bounded however the number is
//!   written — including infinite, which clamps like any other overshoot.
//! - The result is added to a **phase**, and a phase offset only moves where
//!   the sine is read. There is no state that can grow, because there is no
//!   state but two past samples of a sine.
//!
//! So the loop is one sample deep rather than a recursion, and the note it
//! renders always terminates with the buffer it was given.
//!
//! **Two past samples, averaged, rather than one.** That is the standard
//! damping and it is not decoration: reading a single previous sample lets the
//! loop settle into an alternating ±1 at high depths — a hard buzz at half the
//! sample rate that is the same sound at every pitch. Averaging the last two
//! is a one-pole lowpass on the feedback path, which is enough to keep the
//! growl a growl.

use std::f32::consts::PI;

/// The deepest a feedback path may bend its own operator's phase, in radians.
///
/// Half a cycle. Below it a self-modulated sine fattens smoothly toward a saw,
/// which is the useful range; at it the operator has broken up into a rasp,
/// which is the point of having the control at all. There is nothing past it
/// worth reaching — further depth is more noise and not a different sound —
/// and a stated ceiling is what makes
/// [`Operator::feedback`](crate::patch::Operator::feedback) a `0..=1` knob
/// rather than a number a recipe has to calibrate.
pub(crate) const MAX_FEEDBACK: f32 = PI;

/// How far an operator's own last two outputs bend its phase, in radians.
///
/// `last` is the two most recent outputs of this operator, newest first.
pub(crate) fn bend(amount: f32, last: [f32; 2]) -> f32 {
    amount.clamp(0.0, 1.0) * MAX_FEEDBACK * (last[0] + last[1]) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bound, at the extremes of what an operator's output can be and at
    /// every way of writing an amount that is not one — including the ones a
    /// clamp is the only defence against.
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

    /// Both samples are read, and they are averaged rather than summed — so a
    /// path that has just swung from one extreme to the other contributes
    /// nothing, which is the damping the module doc describes.
    #[test]
    fn the_two_stored_samples_are_averaged() {
        assert_eq!(bend(1.0, [1.0, -1.0]), 0.0);
        assert_eq!(bend(1.0, [1.0, 0.0]), MAX_FEEDBACK * 0.5);
        assert_eq!(bend(1.0, [0.0, 1.0]), MAX_FEEDBACK * 0.5);
        assert_eq!(bend(0.5, [1.0, 1.0]), MAX_FEEDBACK * 0.5);
    }
}
