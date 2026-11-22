//! `deny_unknown_fields` does not compose with `#[serde(flatten)]`, so strictness is checked
//! after parsing: the op is serialized back and any key the author wrote that it does not
//! produce is refused by name.
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{Arity, OpKind, OPS};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub op: OpKind,
    #[serde(default)]
    pub inputs: Vec<String>,
}

impl Node {
    /// A missing input would bake a black map; a surplus one would be ignored.
    pub fn check_arity(&self) -> Result<(), NodeError> {
        let accepts = self.op.arity();
        if accepts.accepts(self.inputs.len()) {
            return Ok(());
        }
        Err(self.error(Fault::Arity {
            func: self.op.func(),
            fix: self.op.arity_fix(self.inputs.len()),
            given: self.inputs.len(),
            accepts,
        }))
    }

    pub fn error(&self, fault: Fault) -> NodeError {
        NodeError {
            node: self.id.clone(),
            op: self.op.tag().to_string(),
            fault,
        }
    }
}

/// Rendered as `node "m" (mapping): unknown key "scael"; …`.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeError {
    pub node: String,
    /// The op's tag, or what the author wrote for it when that names no op.
    pub op: String,
    pub fault: Fault,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fault {
    Parse(String),
    Arity {
        /// The `math` func, whose arity narrows the op's (a unary func takes one input).
        func: Option<String>,
        fix: Option<String>,
        given: usize,
        accepts: Arity,
    },
    UnknownInput(String),
    UnknownLink {
        param: String,
        link: String,
    },
    Cycle,
    MissingSource(String),
    MissingFont(String),
    Unreadable {
        path: String,
        error: String,
    },
    Unexpanded(String),
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node {:?} ({}): ", self.node, self.op)?;
        match &self.fault {
            Fault::Parse(message) => f.write_str(message),
            Fault::Arity {
                func,
                fix,
                given,
                accepts,
            } => {
                let noun = if accepts.max == 1 { "input" } else { "inputs" };
                if let Some(func) = func {
                    write!(f, "func {func:?} ")?;
                }
                write!(f, "takes {accepts} {noun}, but {given} given")?;
                fix.as_ref().map_or(Ok(()), |fix| write!(f, "; {fix}"))
            }
            Fault::UnknownInput(input) => write!(f, "references unknown input {input:?}"),
            Fault::UnknownLink { param, link } => {
                write!(f, "links {param} to unknown node {link:?}")
            }
            Fault::Cycle => f.write_str("is on a cycle"),
            Fault::MissingSource(path) => write!(f, "has no image loaded for {path:?}"),
            Fault::MissingFont(path) => write!(f, "has no font loaded for {path:?}"),
            Fault::Unreadable { path, error } => write!(f, "can't read {path:?}: {error}"),
            Fault::Unexpanded(path) => write!(
                f,
                "instances {path:?} but was never expanded; the Texture and Material \
                 verbs load and expand a recipe's graphs before running it"
            ),
        }
    }
}

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

fn parse(given: &Value) -> Result<Node, NodeError> {
    let written = |key: &str| given.get(key).and_then(Value::as_str).unwrap_or("?");
    let at = |message: String| NodeError {
        node: written("id").to_string(),
        op: written("op").to_string(),
        fault: Fault::Parse(message),
    };
    if written("id").contains(['/', '.']) {
        return Err(at(
            "a node id has no '/' or '.': they name a graph's inner nodes \
             (`brick/mortar`) and outputs (`brick.height`)"
                .into(),
        ));
    }
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

fn unknown_key<'a>(given: &'a Value, known: &Value) -> Option<&'a str> {
    let given = given.as_object()?;
    given
        .keys()
        .map(String::as_str)
        .find(|k| !matches!(*k, "id" | "inputs") && known.get(k).is_none())
}

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
