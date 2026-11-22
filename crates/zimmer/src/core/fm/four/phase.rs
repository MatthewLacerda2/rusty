//! where a four-operator note's operators start in their cycles.

use crate::hash::unit2;
use crate::patch::FM_OPERATORS;

/// ASCII "F4", not `two`'s "FM": one note seed reaches both renderers.
const CHANNEL: u64 = 0x4634;

pub(crate) fn starts(seed: u64) -> [f32; FM_OPERATORS] {
    std::array::from_fn(|op| unit2(op as i64, 0, CHANNEL, seed))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_gaps_between_them_are_redrawn_per_note() {
        let gap = |seed| {
            let phases = starts(seed);
            phases[1] - phases[0]
        };
        let moved = (0..64).filter(|seed| (gap(*seed) - gap(0)).abs() > 1e-6);
        assert!(moved.count() > 50, "the gap barely moves between notes");
    }

    #[test]
    fn one_seed_draws_one_set_of_phases_every_time() {
        assert_eq!(starts(11), starts(11), "a phase draw is not reproducible");
        assert_ne!(starts(11), starts(12), "two strikes start in one place");
    }
}
