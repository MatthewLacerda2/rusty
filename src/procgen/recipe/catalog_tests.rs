//! Drift guards: the static op table must agree with `OpKind`'s serde derive.

use serde_json::{json, Value};

use super::*;
use crate::procgen::recipe::{sample_op, OpKind};

/// The tags serde lists when `json` names an unknown one — its "unknown variant"
/// message spells them as `` `a` ``, `` `a` or `b` `` or `` one of `a`, `b`, … ``.
fn expected_tags(json: Value) -> Vec<String> {
    let err = serde_json::from_value::<OpKind>(json)
        .unwrap_err()
        .to_string();
    let list = err.split("expected").nth(1).unwrap_or_default();
    list.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn lit(l: Lit) -> Value {
    match l {
        Lit::Number(n) => json!(n),
        Lit::Integer(i) => json!(i),
        Lit::Bool(b) => json!(b),
        Lit::Text(t) => json!(t),
    }
}

/// The op built from only its required params, serialized back by serde.
fn round_trip(info: &OpInfo) -> serde_json::Map<String, Value> {
    let Value::Object(out) = serde_json::to_value(sample_op(info)).unwrap() else {
        unreachable!()
    };
    out
}

#[test]
fn every_op_kind_variant_has_exactly_one_entry() {
    let mut variants = expected_tags(json!({ "op": "__not_an_op__" }));
    let mut table: Vec<String> = OPS.iter().map(|o| o.op.to_string()).collect();
    assert!(!variants.is_empty(), "serde's variant list wasn't found");
    variants.sort();
    table.sort();
    assert_eq!(table, variants);
}

#[test]
fn every_entry_lists_exactly_its_variants_params() {
    for info in OPS {
        let out = round_trip(info);
        let mut serde_keys: Vec<&str> = out
            .keys()
            .map(String::as_str)
            .filter(|k| *k != "op")
            .collect();
        let mut table_keys: Vec<&str> = info.params.iter().map(|p| p.name).collect();
        serde_keys.sort();
        table_keys.sort();
        assert_eq!(table_keys, serde_keys, "op {}", info.op);
    }
}

#[test]
fn every_default_is_the_one_serde_fills_in() {
    for info in OPS {
        let out = round_trip(info);
        for p in info.params {
            if let Some(d) = p.default {
                assert_eq!(out[p.name], lit(d), "{}.{}", info.op, p.name);
            }
        }
    }
}

#[test]
fn every_enum_param_lists_exactly_the_accepted_values() {
    for info in OPS {
        for p in info.params.iter().filter(|p| p.ty == "enum") {
            let mut doc = round_trip(info);
            doc.insert(p.name.into(), json!("__not_a_value__"));
            let mut accepted = expected_tags(Value::Object(doc));
            let mut listed: Vec<String> = p.values.iter().map(|v| v.to_string()).collect();
            accepted.sort();
            listed.sort();
            assert_eq!(listed, accepted, "{}.{}", info.op, p.name);
        }
    }
}

#[test]
fn every_op_kind_tag_is_the_one_serde_writes() {
    for info in OPS {
        let op = sample_op(info);
        assert_eq!(op.tag(), info.op);
        assert_eq!(op.info().op, info.op);
    }
}

/// `info`'s op with param `name` set to `value`, parsed by serde.
fn with_param(info: &OpInfo, name: &str, value: Value) -> Result<OpKind, serde_json::Error> {
    let mut doc = round_trip(info);
    doc.insert(name.into(), value);
    serde_json::from_value(Value::Object(doc))
}

#[test]
fn every_seed_param_reaches_the_runner() {
    // `OpKind::seed` has a `_ => 0` arm, so the compiler won't flag a new stochastic
    // op missing from it: this does (#1487).
    for info in OPS
        .iter()
        .filter(|i| i.params.iter().any(|p| p.name == "seed"))
    {
        let op = with_param(info, "seed", json!(7)).unwrap();
        assert_eq!(op.seed(), 7, "{}.seed never reaches OpKind::seed", info.op);
    }
}

#[test]
fn every_color_param_takes_srgb_hex() {
    for info in OPS {
        for p in info.params {
            let hex = match p.ty {
                "color" => json!("#000000"),
                "stops" => json!([{ "pos": 0.0, "color": "#000000" }]),
                _ => continue,
            };
            let parsed = with_param(info, p.name, hex);
            assert!(parsed.is_ok(), "{}.{}: {parsed:?}", info.op, p.name);
        }
    }
}
