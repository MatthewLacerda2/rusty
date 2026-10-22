//! The mechanism of [`decode`] on small stand-in types: each test pins one rule the
//! module doc states. The real enums a script reaches are in `enum_case_tests.rs`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Kind {
    Linear,
    Radial,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
    OscStack { wave: Kind },
    Noise,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Doc {
    name: String,
    kind: Kind,
    #[serde(default)]
    more: Vec<Kind>,
    #[serde(default)]
    ops: Vec<Op>,
}

fn doc(json: Value) -> Result<Doc, String> {
    decode(json)
}

#[test]
fn a_variant_name_reads_in_any_case() {
    for given in ["Radial", "radial", "RADIAL", "rAdIaL"] {
        let d = doc(json!({ "name": "g", "kind": given })).unwrap();
        assert_eq!(d.kind, Kind::Radial, "{given}");
    }
}

#[test]
fn tagged_and_nested_enums_read_in_any_case_too() {
    let d = doc(json!({
        "name": "g", "kind": "LINEAR", "more": ["radial", "Linear", "RADIAL"],
        "ops": [{ "op": "OSC_STACK", "wave": "radial" }, { "op": "Noise" }],
    }))
    .unwrap();
    assert_eq!(d.more, [Kind::Radial, Kind::Linear, Kind::Radial]);
    let wave = Kind::Radial;
    assert_eq!(d.ops, [Op::OscStack { wave }, Op::Noise]);
}

#[test]
fn only_the_failing_string_is_rewritten() {
    let d = doc(json!({ "name": "RADIAL", "kind": "RADIAL", "more": ["RADIAL"] })).unwrap();
    assert_eq!(d.name, "RADIAL", "a free string keeps its spelling");
    assert_eq!((d.kind, d.more[0]), (Kind::Radial, Kind::Radial));
    let written = serde_json::to_value(&d).unwrap();
    assert_eq!(written["kind"], "Radial", "written back canonically");
}

#[test]
fn an_unknown_name_fails_exactly_as_plain_serde_does() {
    for bad in [
        json!({ "name": "g", "kind": "Conic" }),
        json!({ "name": "g", "kind": "radial", "more": ["spiral"] }),
        json!({ "name": "g", "kind": "Linear", "ops": [{ "op": "osc stack" }] }),
    ] {
        let plain = serde_json::from_value::<Doc>(bad.clone()).unwrap_err();
        let err = doc(bad).unwrap_err();
        let first = plain.to_string();
        assert!(err.contains("unknown variant"), "{err}");
        if !first.contains("`radial`") {
            assert_eq!(err, first);
        }
    }
}

#[test]
fn other_errors_are_untouched() {
    let bad = json!({ "name": 3, "kind": "radial" });
    let plain = serde_json::from_value::<Doc>(bad.clone()).unwrap_err();
    assert_eq!(doc(bad).unwrap_err(), plain.to_string());
}

#[test]
fn a_name_two_variants_share_ignoring_case_stays_an_error() {
    #[derive(Debug, Deserialize)]
    #[allow(non_camel_case_types, dead_code)]
    enum Twin {
        Up,
        UP,
    }
    let err = decode::<Twin>(json!("up")).unwrap_err();
    assert!(err.contains("unknown variant `up`"), "{err}");
}

#[test]
fn serdes_message_is_parsed_in_all_three_shapes() {
    let one = "unknown variant `x`, expected `A`";
    let two = "unknown variant `x`, expected `A` or `B`";
    let many = "node \"n\": unknown variant `x`, expected one of `A`, `B`, `C`";
    assert_eq!(unknown_variant(one), Some(("x".into(), vec!["A"])));
    assert_eq!(unknown_variant(two), Some(("x".into(), vec!["A", "B"])));
    assert_eq!(
        unknown_variant(many),
        Some(("x".into(), vec!["A", "B", "C"]))
    );
    assert_eq!(unknown_variant("missing field `kind`"), None);
}
