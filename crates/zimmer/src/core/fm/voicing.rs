//! which of the four operators a note actually renders, and what its carriers
//! are divided by.
//!
//! Both are decided **once per note**, before a single sample is produced, and
//! both are here rather than inside the render pass because they are the two
//! questions that have nothing to do with time: an operator is in the note or
//! it is not, and the mix is scaled by one number for the whole of it.
//!
//! They are also the pair of boundary conditions worth stating exactly. An
//! operator is dropped at **either** end — at or below DC, where it names no
//! frequency at all, and at or above Nyquist, where its own sine is already
//! not the sine that was written. Whichever end takes it, it must also leave
//! the normalisation, or a note played high would get quieter purely because
//! more of it had fallen off the top.

use crate::patch::{Algorithm, FM_OPERATORS, Operator};

/// Which operators this note renders, given the largest ratio it can carry.
///
/// **Both bounds are exclusive**, and each for its own reason.
///
/// At the bottom, a ratio of zero is a phase that never advances — a DC offset
/// rather than a pitch — and a negative one names a frequency below it. Only
/// `Patch::validate` can refuse those to a recipe's face, and it does; this is
/// the renderer declining to make a sound out of one that reached it anyway.
///
/// At the top, an operator landing exactly on Nyquist advances exactly half a
/// cycle per sample, so it renders as an alternating ±A whose amplitude is
/// decided by wherever in its cycle the note happened to start — no converter
/// can reproduce it and nobody chose its level, which makes it the same
/// unusable signal as one that folded.
///
/// Both comparisons are false for a `NaN` ratio, which is the right answer for
/// that too.
pub(crate) fn sounding(operators: &[Operator; FM_OPERATORS], ceiling: f32) -> [bool; FM_OPERATORS] {
    std::array::from_fn(|i| operators[i].ratio > 0.0 && operators[i].ratio < ceiling)
}

/// What the summed carriers are divided by: the total level of the carriers
/// that actually sound.
///
/// Dropped operators are left out on purpose: a source whose level fell away
/// as it was played higher, purely because more of it had gone past Nyquist,
/// would fight every gain decision the mix makes, and a note high enough to be
/// a lone carrier would arrive as a whisper rather than as a sine.
///
/// Zero when nothing sounds, which silences the note rather than dividing by
/// nothing.
pub(crate) fn normaliser(
    algorithm: Algorithm,
    levels: &[f32; FM_OPERATORS],
    sounding: &[bool; FM_OPERATORS],
) -> f32 {
    let total: f32 = (0..FM_OPERATORS)
        .filter(|op| sounding[*op] && algorithm.is_carrier(*op))
        .map(|op| levels[op].max(0.0))
        .sum();
    if total > 0.0 { 1.0 / total } else { 0.0 }
}

/// The two boundaries by the numbers, which is the only way to state them:
/// every value here is either exactly on a bound or one step off it, so no
/// tolerance stands between the assertion and the comparison it is about.
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

    /// Whether a lone operator at `ratio` would be rendered under `ceiling`.
    fn sounds(ratio: f32, ceiling: f32) -> bool {
        sounding(&[op(ratio), op(1.0), op(1.0), op(1.0)], ceiling)[0]
    }

    /// The bottom bound is exclusive: zero is a phase that never advances, not
    /// the lowest legal pitch.
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

    /// The top bound is exclusive too, and the operator sitting *exactly* on
    /// Nyquist is the one that decides it: half a cycle per sample is not a
    /// tone anything can reproduce.
    #[test]
    fn an_operator_exactly_on_nyquist_is_dropped_like_one_above_it() {
        assert!(!sounds(4.0, 4.0), "exactly on the line");
        assert!(!sounds(4.5, 4.0), "and past it");
        assert!(sounds(3.999, 4.0), "just under it still sounds");
    }

    /// A ceiling of zero — a note with no pitch to place anything against —
    /// drops every operator there is, including one at DC.
    #[test]
    fn nothing_sounds_against_a_ceiling_of_zero() {
        for ratio in [0.0, 1.0, 1e9, f32::MAX] {
            assert!(!sounds(ratio, 0.0), "{ratio}");
        }
    }

    /// The mix is divided by the carriers that sound, so adding a carrier
    /// changes the balance rather than the volume — and a carrier that was
    /// dropped is not in the sum, which is what stops a high note also being a
    /// quiet one.
    #[test]
    fn only_the_carriers_that_sound_are_divided_by() {
        let all = [true; FM_OPERATORS];
        let levels = [3.0, 1.0, 5.0, 3.0];
        // Twin hears operators 2 and 4: 1 + 3 = 4.
        assert_eq!(normaliser(Algorithm::Twin, &levels, &all), 0.25);
        // Chain hears only operator 4, whatever the modulators are set to.
        assert_eq!(normaliser(Algorithm::Chain, &levels, &all), 1.0 / 3.0);
        // Drop operator 4 and twin is left dividing by operator 2 alone.
        let without = [true, true, true, false];
        assert_eq!(normaliser(Algorithm::Twin, &levels, &without), 1.0);
        // A modulator's level never joins the total, dropped or not.
        let quiet = [true, true, false, true];
        assert_eq!(normaliser(Algorithm::Twin, &levels, &quiet), 0.25);
    }

    /// Carriers with nothing but zero between them divide by nothing rather
    /// than by zero — the note is silent, not `NaN`. A negative level is the
    /// same case, since it is floored before it is summed.
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
