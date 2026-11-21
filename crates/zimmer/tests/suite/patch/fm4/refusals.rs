//! What `fm4` validation refuses: a ratio that is not a positive number, and a routing with no
//! sounding carrier.

use crate::common::minimal;
use rusty_zimmer::patch::{Algorithm, Source};
use rusty_zimmer::SynthError;

use super::op;

#[test]
fn a_ratio_that_is_not_a_positive_number_is_refused_by_the_operator_it_is_on() {
    for (index, ratio) in [
        (1, 0.0),
        (2, -1.0),
        (3, f32::NAN),
        (4, f32::INFINITY),
        (4, 0.0),
    ] {
        let mut operators = [op(1.0), op(2.0), op(3.0), op(4.0)];
        operators[index - 1].ratio = ratio;
        let patch = minimal(Source::Fm4 {
            algorithm: Algorithm::Chain,
            operators,
            vel_index: 0.0,
        });
        match patch.validate() {
            // Compared field by field because `NaN != NaN`, so the whole error is not equal to
            // itself and `assert_eq!` would pass for a check that never fired.
            Err(SynthError::BadOperatorRatio { operator, ratio: r }) => {
                assert_eq!(operator, index);
                assert_eq!(r.is_nan(), ratio.is_nan());
                assert!(r.is_nan() || r == ratio, "reported {r} for {ratio}");
            }
            other => panic!("operator {index} at ratio {ratio} gave {other:?}"),
        }
    }
}

#[test]
fn an_algorithm_with_every_carrier_at_zero_is_refused() {
    let mut operators = [op(1.0), op(2.0), op(3.0), op(1.0)];
    operators[3].level = 0.0;
    let silent = |algorithm| {
        minimal(Source::Fm4 {
            algorithm,
            operators,
            vel_index: 0.0,
        })
        .validate()
    };
    assert_eq!(
        silent(Algorithm::Chain),
        Err(SynthError::SilentCarriers { algorithm: "chain" })
    );
    // The same operators under a routing with a second carrier still sound, which is the whole
    // point of the check being about the algorithm.
    assert_eq!(silent(Algorithm::Twin), Ok(()));
}
