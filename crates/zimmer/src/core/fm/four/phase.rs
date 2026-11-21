//! where a four-operator note's operators start in their cycles.
//!
//! Four draws from the note's seed, one per operator. The argument is the
//! oscillator stack's — see [`osc`](crate::core::osc), which carries it in
//! full, including why there is deliberately no way to hard-sync a source and
//! what the field would look like the day a patch needs one — and it is
//! [`two`](super::super::two)'s second-draw argument multiplied. Two operators
//! stand in one relationship; four stand in **six**, and in FM those
//! relationships are not decoration on the timbre, they *are* it: which
//! sidebands the first milliseconds are built from is decided by where each
//! operator was in its cycle when the note began.
//!
//! `fm4` is also where the sustained, layered sounds live — brass, pads,
//! basses, a bell rung into a body — which is exactly the material a song
//! plays over and over. A start that never moved would make every one of those
//! repeats a photocopy of the first, and phase-locked notes stacked into a
//! chord reinforce and cancel at the same points every time, which is a
//! comb-filtered chord rather than a chord.
//!
//! One draw shared between the four would move where the note begins without
//! moving any of the six relationships, so a second strike would still be the
//! same attack. That is why this is four draws and not one.

use crate::hash::unit2;
use crate::patch::FM_OPERATORS;

/// Hash channel the four start phases draw on.
///
/// Its own, and not [`two`](super::super::two)'s: one note seed reaches both
/// renderers, and there is no reason for two of a four-operator patch's
/// operators to begin at exactly the two places a two-operator patch's
/// carrier and modulator would have. Distinct from the noise, Karplus,
/// oscillator-stack and additive channels for the same reason.
const CHANNEL: u64 = 0x4634; // "F4"

/// Where each operator of a note seeded `seed` starts in its cycle, in `0..1`.
///
/// The operator index is the lattice coordinate, which is what makes the four
/// draws independent of each other rather than one draw offset four ways.
pub(crate) fn starts(seed: u64) -> [f32; FM_OPERATORS] {
    std::array::from_fn(|op| unit2(op as i64, 0, CHANNEL, seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every draw is a phase, and the four are drawn **apart**. One number
    /// shared between the operators would pass a determinism test and a
    /// two-seeds-differ test alike, while leaving the six relationships the
    /// timbre is made of exactly where they were.
    #[test]
    fn the_four_operators_start_at_four_different_places() {
        for seed in 0..64 {
            let phases = starts(seed);
            for (op, phase) in phases.iter().enumerate() {
                assert!((0.0..1.0).contains(phase), "operator {op}: {phase}");
            }
            for a in 0..FM_OPERATORS {
                for b in (a + 1)..FM_OPERATORS {
                    assert_ne!(phases[a], phases[b], "operators {a}/{b}, seed {seed}");
                }
            }
        }
    }

    /// And the relationships themselves are re-drawn per note rather than
    /// carried to the next one intact: the gap between two operators is a
    /// different gap on the next strike, which is the whole of what the four
    /// separate draws buy over one.
    #[test]
    fn the_gaps_between_them_are_redrawn_per_note() {
        let gap = |seed| {
            let phases = starts(seed);
            phases[1] - phases[0]
        };
        let moved = (0..64).filter(|seed| (gap(*seed) - gap(0)).abs() > 1e-6);
        assert!(moved.count() > 50, "the gap barely moves between notes");
    }

    /// The claim `generated/` rests on, at this source: a draw is a pure
    /// function of the seed, and a different seed is a different note.
    #[test]
    fn one_seed_draws_one_set_of_phases_every_time() {
        assert_eq!(starts(11), starts(11), "a phase draw is not reproducible");
        assert_ne!(starts(11), starts(12), "two strikes start in one place");
    }
}
