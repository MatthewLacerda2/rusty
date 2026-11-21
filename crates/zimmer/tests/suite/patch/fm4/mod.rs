//! `fm4` as a document: what a recipe may leave out, and the one shape the parser settles before
//! validation ever runs; what validation refuses is in `refusals`.

use crate::common::minimal;
use rusty_zimmer::patch::{Adsr, Algorithm, Operator, Source, FM_OPERATORS};
use rusty_zimmer::Patch;

mod refusals;

fn op(ratio: f32) -> Operator {
    Operator {
        ratio,
        level: 1.0,
        feedback: 0.0,
        env: None,
    }
}

fn fm4(algorithm: Algorithm) -> Source {
    Source::Fm4 {
        algorithm,
        operators: [op(1.0), op(2.0), op(3.0), op(1.0)],
        vel_index: 0.0,
    }
}

#[test]
fn every_algorithm_is_playable() {
    for algorithm in Algorithm::ALL {
        minimal(fm4(algorithm))
            .validate()
            .unwrap_or_else(|error| panic!("{}: {error}", algorithm.name()));
    }
}

#[test]
fn the_source_is_spelled_fm4() {
    assert_eq!(fm4(Algorithm::Twin).kind(), "fm4");
}

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
