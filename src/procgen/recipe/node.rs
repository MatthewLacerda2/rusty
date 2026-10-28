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

use super::{Arity, OpKind};

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

impl Node {
    /// Refuse an `inputs` list its op does not accept: a missing input would bake a
    /// black or flat map, a surplus one would be silently ignored.
    pub fn check_arity(&self) -> Result<(), ArityError> {
        let accepts = self.op.arity();
        if accepts.accepts(self.inputs.len()) {
            return Ok(());
        }
        let func = match &self.op {
            OpKind::Math(p) => serde_json::to_value(p.func).ok(),
            _ => None,
        };
        Err(ArityError {
            node: self.id.clone(),
            op: self.op.tag(),
            func: func.and_then(|f| f.as_str().map(str::to_string)),
            given: self.inputs.len(),
            accepts,
        })
    }
}

/// A node wired with more or fewer inputs than its op accepts.
#[derive(Clone, Debug, PartialEq)]
pub struct ArityError {
    pub node: String,
    pub op: &'static str,
    /// The `math` func, whose arity narrows the op's (a unary func takes one input).
    pub func: Option<String>,
    pub given: usize,
    pub accepts: Arity,
}

impl std::fmt::Display for ArityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            node,
            op,
            func,
            given,
            accepts,
        } = self;
        let noun = if accepts.max == 1 { "input" } else { "inputs" };
        let func = func
            .as_ref()
            .map(|f| format!(", func {f:?}"))
            .unwrap_or_default();
        write!(
            f,
            "node {node:?} (op {op:?}{func}) takes {accepts} {noun}, but {given} given"
        )
    }
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
        node.check_arity().map_err(D::Error::custom)?;
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
    fn a_wrong_input_count_is_refused_at_parse_time() {
        let short = parse(r#"{ "id": "m", "op": "mix", "mode": "add", "inputs": ["a"] }"#);
        let err = short.unwrap_err();
        assert!(err.contains("\"m\"") && err.contains("1 given"), "{err}");
        let surplus = parse(r#"{ "id": "i", "op": "invert", "inputs": ["a", "b"] }"#);
        assert!(surplus.unwrap_err().contains("2 given"));
    }

    #[test]
    fn a_unary_math_func_refuses_a_second_input() {
        let math = |func: &str, inputs: &str| {
            parse(&format!(
                r#"{{ "id": "m", "op": "math", "func": "{func}", "inputs": [{inputs}] }}"#
            ))
        };
        for func in ["abs", "fract", "sqrt"] {
            let err = math(func, r#""a", "b""#).unwrap_err();
            assert!(
                err.contains("\"m\"") && err.contains(&format!("func \"{func}\"")),
                "{err}"
            );
            assert!(err.contains("takes 1 input, but 2 given"), "{err}");
            assert!(math(func, r#""a""#).is_ok(), "{func}");
        }
        assert!(math("max", r#""a", "b""#).is_ok());
    }

    #[test]
    fn a_misspelled_ramp_stop_key_is_refused() {
        let json =
            r#"{ "id": "r", "op": "color_ramp", "stops": [{ "pos": 0, "colour": [0,0,0,1] }] }"#;
        assert!(parse(json).unwrap_err().contains("colour"));
    }
}
