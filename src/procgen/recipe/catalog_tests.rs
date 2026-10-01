//! Drift guards: the static op table must agree with `OpKind`'s serde derive.

use serde_json::{json, Value};

use super::*;
use crate::procgen::recipe::OpKind;

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

/// A valid value for a required param of type `p.ty`.
fn sample(p: &OpParam) -> Value {
    match p.ty {
        "number" | "integer" => json!(2),
        "bool" => json!(true),
        "color" => json!([0.0, 0.0, 0.0, 1.0]),
        "vec2" => json!([1.0, 1.0]),
        "enum" => json!(p.values[0]),
        "stops" => json!([{ "pos": 0.0, "color": [0.0, 0.0, 0.0, 1.0] }]),
        other => panic!("{}: unknown param type {other}", p.name),
    }
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
    let mut doc = serde_json::Map::new();
    doc.insert("op".into(), json!(info.op));
    for p in info.params.iter().filter(|p| p.default.is_none()) {
        doc.insert(p.name.into(), sample(p));
    }
    let op: OpKind = serde_json::from_value(Value::Object(doc))
        .unwrap_or_else(|e| panic!("{}: required params don't parse: {e}", info.op));
    let Value::Object(out) = serde_json::to_value(op).unwrap() else {
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
