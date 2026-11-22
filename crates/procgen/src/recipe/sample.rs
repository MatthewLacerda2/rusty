//! Test support: a valid op for every catalog row, built from its required params.
//! Compiled in every build for [`sample_required`], a test hook another crate's
//! tests call; the rest is `cfg(test)`.

use serde_json::{json, Value};

#[cfg(test)]
use super::OpInfo;
#[cfg(test)]
use super::OpKind;
use super::{OpParam, SAMPLE_PATH};

/// Test hook for the script crate's enum-case test: the row's [`OpParam::sample`], else its type's.
#[doc(hidden)]
pub fn sample_required(p: &OpParam) -> Value {
    p.sample.map_or_else(|| sample(p), |v| json!(v))
}

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

#[cfg(test)]
pub fn sample_op(info: &OpInfo) -> OpKind {
    let mut doc = serde_json::Map::new();
    doc.insert("op".into(), json!(info.op));
    for p in info.params.iter().filter(|p| p.default.is_none()) {
        doc.insert(p.name.into(), sample_required(p));
    }
    serde_json::from_value(Value::Object(doc))
        .unwrap_or_else(|e| panic!("{}: required params don't parse: {e}", info.op))
}

/// Params a row's defaults leave unread, so its sample reads every input and socket: at
/// `expr`'s default of 0 an `expression` would take no inputs.
#[cfg(test)]
const READING: &[(&str, &str, &str)] =
    &[("expression", "expr", "a + b * c - d + k1 + k2 * k3 - k4")];

/// [`sample_op`] with [`READING`] written, for tests that wire inputs or link params.
#[cfg(test)]
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
