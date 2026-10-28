//! Test support: a valid op for every catalog row, built from its required params.

use serde_json::{json, Value};

use super::{OpInfo, OpKind, OpParam};

/// The file a sampled `path` param names; tests that evaluate a sampled `image` op
/// hand procgen a source under this key.
pub const SAMPLE_PATH: &str = "textures/sample.png";

/// A valid value for a required param of type `p.ty`.
fn sample(p: &OpParam) -> Value {
    match p.ty {
        "number" | "integer" => json!(2),
        "bool" => json!(true),
        "color" => json!([0.0, 0.0, 0.0, 1.0]),
        "vec2" => json!([1.0, 1.0]),
        "enum" => json!(p.values[0]),
        "path" => json!(SAMPLE_PATH),
        "stops" => json!([{ "pos": 0.0, "color": [0.0, 0.0, 0.0, 1.0] }]),
        other => panic!("{}: unknown param type {other}", p.name),
    }
}

/// `info`'s op with its required params filled in and the rest at their defaults.
pub fn sample_op(info: &OpInfo) -> OpKind {
    let mut doc = serde_json::Map::new();
    doc.insert("op".into(), json!(info.op));
    for p in info.params.iter().filter(|p| p.default.is_none()) {
        doc.insert(p.name.into(), sample(p));
    }
    serde_json::from_value(Value::Object(doc))
        .unwrap_or_else(|e| panic!("{}: required params don't parse: {e}", info.op))
}
