//! `fm4` as a document: what a recipe may leave out, what is refused, and the
//! one shape the parser settles before validation ever runs.

use crate::common::minimal;
use scorsese_zimmer::patch::{Adsr, Algorithm, FM_OPERATORS, Operator, Source};
use scorsese_zimmer::{Patch, SynthError};

/// A plain operator at `ratio`: full level, no feedback, no envelope of its
/// own.
fn op(ratio: f32) -> Operator {
    Operator {
        ratio,
        level: 1.0,
        feedback: 0.0,
        env: None,
    }
}

/// A legal four-operator source under `algorithm`.
fn fm4(algorithm: Algorithm) -> Source {
    Source::Fm4 {
        algorithm,
        operators: [op(1.0), op(2.0), op(3.0), op(1.0)],
        vel_index: 0.0,
    }
}

/// Every routing renders, and the source is a source like any other — nothing
/// about the algorithm is a special case anywhere downstream.
#[test]
fn every_algorithm_is_playable() {
    for algorithm in Algorithm::ALL {
        minimal(fm4(algorithm))
            .validate()
            .unwrap_or_else(|error| panic!("{}: {error}", algorithm.name()));
    }
}

/// The word the document is written with is the word a report and an error use
/// — the rule `Source::kind` already follows, extended to the new source.
#[test]
fn the_source_is_spelled_fm4() {
    assert_eq!(fm4(Algorithm::Twin).kind(), "fm4");
}

/// What a recipe may leave out of an operator: everything but the ratio. A
/// level of one and no envelope is a plain sine at full weight, which is the
/// operator somebody writing their first `fm4` means.
#[test]
fn an_operator_needs_only_its_ratio() {
    let json = r#"{
        "kind": "fm4",
        "algorithm": "twin",
        "operators": [
            { "ratio": 1.0 }, { "ratio": 2.0 },
            { "ratio": 1.0 }, { "ratio": 3.0 }
        ]
    }"#;
    let source: Source = serde_json::from_str(json).expect("the shortest form parses");
    let Source::Fm4 {
        operators,
        vel_index,
        algorithm,
    } = source
    else {
        panic!("parsed as something other than fm4");
    };
    assert_eq!(algorithm, Algorithm::Twin);
    assert_eq!(vel_index, 0.0, "velocity reaches nothing unless asked");
    for operator in operators {
        assert_eq!(operator.level, 1.0);
        assert_eq!(operator.feedback, 0.0);
        assert_eq!(operator.env, None, "an operator holds its level by default");
    }
}

/// Four is not a suggestion: the count is settled by the parser, before any
/// validation runs, so a list of three cannot reach the renderer with a
/// stand-in operator invented for it.
#[test]
fn a_list_that_is_not_four_operators_does_not_parse() {
    for operators in ["[]", r#"[{ "ratio": 1 }, { "ratio": 2 }, { "ratio": 3 }]"#] {
        let json =
            format!(r#"{{ "kind": "fm4", "algorithm": "chain", "operators": {operators} }}"#);
        assert!(
            serde_json::from_str::<Source>(&json).is_err(),
            "{operators} should not parse as {FM_OPERATORS} operators"
        );
    }
}

/// A patch round-trips through JSON with every operator field carried, and an
/// operator that has no envelope does not grow a `"env": null` on the way.
#[test]
fn a_written_operator_round_trips_and_a_bare_one_stays_bare() {
    let source = Source::Fm4 {
        algorithm: Algorithm::PairAndTwo,
        operators: [
            Operator {
                ratio: 1.41,
                level: 6.5,
                feedback: 0.7,
                env: Some(Adsr {
                    a: 0.001,
                    d: 0.2,
                    s: 0.0,
                    r: 0.05,
                    curve: 3.0,
                }),
            },
            op(1.0),
            op(2.0),
            op(4.0),
        ],
        vel_index: 5.0,
    };
    let patch = minimal(source);
    let json = patch.to_json().expect("a patch serialises");
    assert!(!json.contains("env\": null"), "{json}");
    assert_eq!(Patch::from_json(&json).expect("and parses back"), patch);
}

/// A ratio is a multiple of the played pitch, so anything that is not a
/// positive number describes no sound — and the error says *which* of the
/// four, because four ratios is exactly where an unlabelled complaint sends a
/// reader hunting. Counted from **one**, unlike an additive partial's index,
/// because an operator's number is a row of the algorithm diagram rather than
/// a place in a list.
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
            // Compared field by field because `NaN != NaN`, so the whole
            // error is not equal to itself and `assert_eq!` would pass for a
            // check that never fired.
            Err(SynthError::BadOperatorRatio { operator, ratio: r }) => {
                assert_eq!(operator, index);
                assert_eq!(r.is_nan(), ratio.is_nan());
                assert!(r.is_nan() || r == ratio, "reported {r} for {ratio}");
            }
            other => panic!("operator {index} at ratio {ratio} gave {other:?}"),
        }
    }
}

/// Which operators are audible is a property of the algorithm, so a recipe can
/// silence the whole source by zeroing one operator it happens to be heard
/// through. That is refused, and the message names the routing.
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
    // The same operators under a routing with a second carrier still sound,
    // which is the whole point of the check being about the algorithm.
    assert_eq!(silent(Algorithm::Twin), Ok(()));
}
