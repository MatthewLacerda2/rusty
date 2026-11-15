//! Test support: a valid op for every catalog row, built from its required params.

use serde_json::{json, Value};

use super::{OpInfo, OpKind, OpParam, SAMPLE_PATH};

/// A valid value for a required param of type `p.ty`.
fn sample(p: &OpParam) -> Value {
    match p.ty {
        "number" | "integer" => json!(2),
        "bool" => json!(true),
        "color" => json!([0.0, 0.0, 0.0, 1.0]),
        "vec2" => json!([1.0, 1.0]),
        "enum" => json!(p.values[0]),
        "path" => json!(SAMPLE_PATH),
        "text" => json!("A"),
        "stops" => json!([{ "pos": 0.0, "color": [0.0, 0.0, 0.0, 1.0] }]),
        other => panic!("{}: unknown param type {other}", p.name),
    }
}

/// `info`'s op with its required params filled in and the rest at their defaults.
pub fn sample_op(info: &OpInfo) -> OpKind {
    let mut doc = serde_json::Map::new();
    doc.insert("op".into(), json!(info.op));
    for p in info.params.iter().filter(|p| p.default.is_none()) {
        let given = REQUIRED
            .iter()
            .find(|(op, name, _)| *op == info.op && *name == p.name);
        doc.insert(
            p.name.into(),
            given.map_or_else(|| sample(p), |(.., v)| json!(v)),
        );
    }
    serde_json::from_value(Value::Object(doc))
        .unwrap_or_else(|e| panic!("{}: required params don't parse: {e}", info.op))
}

/// Required params whose type's sample is no valid value: a `path`'s `d` is SVG
/// path data, not any text.
const REQUIRED: &[(&str, &str, &str)] = &[("path", "d", "M0.25 0.25 L0.75 0.25 L0.5 0.75 Z")];

/// Params a row's defaults leave unread, written so its sample reads every input and
/// socket: an `expression` names the inputs it takes in its formula, and at `expr`'s
/// default of 0 it would take none and read no `k`.
const READING: &[(&str, &str, &str)] =
    &[("expression", "expr", "a + b * c - d + k1 + k2 * k3 - k4")];

/// [`sample_op`], with [`READING`]'s params written: the op at its widest wiring and
/// reading every socket, for the tests that wire inputs or link params.
pub fn reading_sample_op(info: &OpInfo) -> OpKind {
    let Value::Object(mut doc) = serde_json::to_value(sample_op(info)).expect("serializes") else {
        unreachable!("an op serializes to an object")
    };
    for (_, name, value) in READING.iter().filter(|(op, ..)| *op == info.op) {
        doc.insert((*name).into(), json!(value));
    }
    serde_json::from_value(Value::Object(doc))
        .unwrap_or_else(|e| panic!("{}: its reading sample doesn't parse: {e}", info.op))
}
