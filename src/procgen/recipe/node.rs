//! src/procgen/recipe/node.rs — one recipe DAG node, parsed strictly (#395).
//!
//! A node flattens its [`OpKind`] params beside `id` and `inputs`, and serde's
//! `deny_unknown_fields` does not compose with `#[serde(flatten)]` — so the check
//! runs after parsing instead: the parsed op is serialized back, and any key the
//! author wrote that it does not produce is refused by name. The op's own serde
//! derive stays the single source of which params exist. A link on a param the
//! catalog doesn't mark `socket` is refused first, naming the ones that take one.
//! Every refusal is a [`NodeError`], which names the node's id and op (#1575).

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{Arity, OpKind, OPS};

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
    pub fn check_arity(&self) -> Result<(), NodeError> {
        let accepts = self.op.arity();
        if accepts.accepts(self.inputs.len()) {
            return Ok(());
        }
        let func = match &self.op {
            OpKind::Math(p) => serde_json::to_value(p.func).ok(),
            _ => None,
        };
        Err(self.error(Fault::Arity {
            func: func.and_then(|f| f.as_str().map(str::to_string)),
            given: self.inputs.len(),
            accepts,
        }))
    }

    /// `fault`, named by this node's id and op.
    pub fn error(&self, fault: Fault) -> NodeError {
        NodeError {
            node: self.id.clone(),
            op: self.op.tag().to_string(),
            fault,
        }
    }
}

/// Something wrong with one node, named by its id and op, as
/// `node "m" (mapping): unknown key "scael"; …`. Every recipe error about a node,
/// from parsing it or from wiring it into the graph, is one of these (#1575), so
/// the prefix is written here once, never per op or per check.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeError {
    pub node: String,
    /// The op's tag, or what the author wrote for it when that names no op.
    pub op: String,
    pub fault: Fault,
}

/// What is wrong with a [`NodeError`]'s node.
#[derive(Clone, Debug, PartialEq)]
pub enum Fault {
    /// The node did not parse: a missing or mistyped param, an unknown key or op.
    Parse(String),
    /// Wired with more or fewer inputs than its op accepts.
    Arity {
        /// The `math` func, whose arity narrows the op's (a unary func takes one input).
        func: Option<String>,
        given: usize,
        accepts: Arity,
    },
    /// An input id no node defines.
    UnknownInput(String),
    /// A param linked to an id no node defines (#1494).
    UnknownLink { param: String, link: String },
    /// The node is on a cycle (the graph is not a DAG).
    Cycle,
    /// An `image` node whose file was not handed in (#1491).
    MissingSource(String),
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node {:?} ({}): ", self.node, self.op)?;
        match &self.fault {
            Fault::Parse(message) => f.write_str(message),
            Fault::Arity {
                func,
                given,
                accepts,
            } => {
                let noun = if accepts.max == 1 { "input" } else { "inputs" };
                if let Some(func) = func {
                    write!(f, "func {func:?} ")?;
                }
                write!(f, "takes {accepts} {noun}, but {given} given")
            }
            Fault::UnknownInput(input) => write!(f, "references unknown input {input:?}"),
            Fault::UnknownLink { param, link } => {
                write!(f, "links {param} to unknown node {link:?}")
            }
            Fault::Cycle => f.write_str("is on a cycle"),
            Fault::MissingSource(path) => write!(f, "has no image loaded for {path:?}"),
        }
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
        parse(&given).map_err(D::Error::custom)
    }
}

/// `given` as a [`Node`], every refusal named by the node's id and op as written.
fn parse(given: &Value) -> Result<Node, NodeError> {
    let written = |key: &str| given.get(key).and_then(Value::as_str).unwrap_or("?");
    let at = |message: String| NodeError {
        node: written("id").to_string(),
        op: written("op").to_string(),
        fault: Fault::Parse(message),
    };
    if let Some(message) = refused_link(given) {
        return Err(at(message));
    }
    let raw: RawNode = serde_json::from_value(given.clone()).map_err(|e| at(e.to_string()))?;
    let node = Node {
        id: raw.id,
        op: raw.op,
        inputs: raw.inputs,
    };
    let known = serde_json::to_value(&node.op).map_err(|e| at(e.to_string()))?;
    if let Some(key) = unknown_key(given, &known) {
        return Err(at(unknown_key_message(&known, key)));
    }
    node.check_arity()?;
    Ok(node)
}

/// A link on a param that takes none (#1494), named with the params of its op that
/// do: "param "scale" takes no link; noise links gain". Only
/// a link is an object among a param's values, so any object counts as one.
fn refused_link(given: &Value) -> Option<String> {
    let node = given.as_object()?;
    let op = node.get("op")?.as_str()?;
    let info = OPS.iter().find(|info| info.op == op)?;
    let refuses = |key: &str| info.params.iter().any(|p| p.name == key && !p.socket);
    let (key, _) = node
        .iter()
        .find(|(key, value)| value.is_object() && refuses(key))?;
    let sockets: Vec<&str> = info
        .params
        .iter()
        .filter(|p| p.socket)
        .map(|p| p.name)
        .collect();
    let links = match sockets.is_empty() {
        true => format!("{op} takes no links"),
        false => format!("{op} links {}", sockets.join(", ")),
    };
    Some(format!("param {key:?} takes no link; {links}"))
}

/// The first key in `given` that is neither a node field nor produced by the op.
fn unknown_key<'a>(given: &'a Value, known: &Value) -> Option<&'a str> {
    let given = given.as_object()?;
    given
        .keys()
        .map(String::as_str)
        .find(|k| !matches!(*k, "id" | "inputs") && known.get(k).is_none())
}

/// "unknown key "scael"; noise takes kind, octaves, scale".
fn unknown_key_message(known: &Value, key: &str) -> String {
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
    format!("unknown key {key:?}; {op} takes {takes} (besides id, op, inputs)")
}

#[cfg(test)]
#[path = "node_tests.rs"]
mod tests;
