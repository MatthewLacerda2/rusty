//! src/procgen/recipe/node.rs — one recipe DAG node, parsed strictly (#395).
//!
//! A node flattens its [`OpKind`] params beside `id` and `inputs`, and serde's
//! `deny_unknown_fields` does not compose with `#[serde(flatten)]` — so the check
//! runs after parsing instead: the parsed op is serialized back, and any key the
//! author wrote that it does not produce is refused by name. The op's own serde
//! derive stays the single source of which params exist.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::OpKind;

/// One node in the recipe DAG: a stable `id`, the op to run, and the `id`s of the
/// input nodes feeding it (in order).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub op: OpKind,
    #[serde(default)]
    pub inputs: Vec<String>,
}

/// The lenient derive the strict [`Node`] deserializer starts from.
#[derive(Deserialize)]
struct RawNode {
    id: String,
    #[serde(flatten)]
    op: OpKind,
    #[serde(default)]
    inputs: Vec<String>,
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let given = Value::deserialize(d)?;
        let raw: RawNode = serde_json::from_value(given.clone()).map_err(D::Error::custom)?;
        let node = Node {
            id: raw.id,
            op: raw.op,
            inputs: raw.inputs,
        };
        let known = serde_json::to_value(&node.op).map_err(D::Error::custom)?;
        if let Some(key) = unknown_key(&given, &known) {
            return Err(D::Error::custom(unknown_key_message(&node.id, &known, key)));
        }
        Ok(node)
    }
}

/// The first key in `given` that is neither a node field nor produced by the op.
fn unknown_key<'a>(given: &'a Value, known: &Value) -> Option<&'a str> {
    let given = given.as_object()?;
    given
        .keys()
        .map(String::as_str)
        .find(|k| !matches!(*k, "id" | "inputs") && known.get(k).is_none())
}

/// "node "n0" (op "noise"): unknown key "scael"; noise takes kind, octaves, scale".
fn unknown_key_message(id: &str, known: &Value, key: &str) -> String {
    let op = known.get("op").and_then(Value::as_str).unwrap_or("?");
    let params: Vec<&str> = known
        .as_object()
        .map(|m| {
            m.keys()
                .map(String::as_str)
                .filter(|k| *k != "op")
                .collect()
        })
        .unwrap_or_default();
    let takes = if params.is_empty() {
        "no params".to_string()
    } else {
        params.join(", ")
    };
    format!(
        "node {id:?} (op {op:?}): unknown key {key:?}; {op} takes {takes} (besides id, op, inputs)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<Node, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    #[test]
    fn a_misspelled_op_param_is_refused_by_name() {
        let err = parse(r#"{ "id": "n", "op": "noise", "kind": "fbm", "scale": 8, "scael": 8 }"#)
            .unwrap_err();
        assert!(err.contains("\"scael\""), "{err}");
        assert!(err.contains("scale") && err.contains("octaves"), "{err}");
    }

    #[test]
    fn a_param_on_a_paramless_op_is_refused() {
        let err = parse(r#"{ "id": "n", "op": "invert", "amount": 1 }"#).unwrap_err();
        assert!(
            err.contains("\"amount\"") && err.contains("no params"),
            "{err}"
        );
    }

    #[test]
    fn known_keys_and_omitted_defaults_still_parse() {
        let node = parse(r#"{ "id": "v", "op": "voronoi", "scale": 4, "inputs": [] }"#);
        assert!(node.is_ok(), "{node:?}");
    }

    #[test]
    fn a_misspelled_ramp_stop_key_is_refused() {
        let json =
            r#"{ "id": "r", "op": "color_ramp", "stops": [{ "pos": 0, "colour": [0,0,0,1] }] }"#;
        assert!(parse(json).unwrap_err().contains("colour"));
    }
}
