//! crates/procgen/src/graph/bind.rs — [`TextureRecipe`]'s serde (#1744): a node value
//! written `{ param = "age" }` is read at the param's default and remembered as a
//! [`Bound`], and written back as the placeholder, so a graph survives a parse and
//! a save (`Texture.ToJson`) with its interface intact.
//!
//! Only a node's own top-level values bind: a whole param (a colour, an enum, a
//! number, a seed), never part of one. An instance's `params` map keeps its
//! placeholders as written: they are values for the graph it instances, filled in
//! by [`super::expand`] from the recipe's own. An instance's `path` takes no
//! placeholder: the graph files are read before any param has a value.

use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use std::collections::BTreeMap;

use super::params::ParamDecl;
use super::placeholder;
use crate::recipe::{power_of_two_sides, Node, TextureRecipe};

/// One node value that is a placeholder: node `node`'s `key` takes param `param`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub node: String,
    pub key: String,
    pub param: String,
}

/// The keys of a node that are its wiring, never a param value.
const WIRING: [&str; 3] = ["id", "op", "inputs"];

/// [`TextureRecipe`] as read, field for field: its nodes stay JSON until their
/// placeholders are bound. Typed fields (not one `Value` for the whole recipe), so
/// the scene format's tracer still sees every field a stored recipe holds.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Doc {
    #[serde(default)]
    resolution: u32,
    #[serde(default, deserialize_with = "power_of_two_sides")]
    size: Option<[u32; 2]>,
    #[serde(default)]
    seed: u64,
    nodes: Vec<Value>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    outputs: BTreeMap<String, String>,
    #[serde(default)]
    params: BTreeMap<String, ParamDecl>,
    #[serde(default)]
    inputs: Vec<String>,
}

impl<'de> Deserialize<'de> for TextureRecipe {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut doc = Doc::deserialize(d)?;
        let bound = bind(&mut doc.nodes, &doc.params).map_err(D::Error::custom)?;
        let nodes = doc.nodes.into_iter().map(Node::deserialize);
        let recipe = TextureRecipe {
            resolution: doc.resolution,
            size: doc.size,
            seed: doc.seed,
            nodes: nodes.collect::<Result<_, _>>().map_err(D::Error::custom)?,
            output: doc.output,
            outputs: doc.outputs,
            params: doc.params,
            inputs: doc.inputs,
            bound,
        };
        check_inputs(&recipe).map_err(D::Error::custom)?;
        Ok(recipe)
    }
}

impl Serialize for TextureRecipe {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.bound.is_empty() {
            return TextureRecipe::serialize(self, s);
        }
        // Through JSON text, not `serde_json::to_value`: that widens an `f32` to
        // its f64 digits (0.1 → 0.10000000149011612); text keeps the short form.
        let mut text = Vec::new();
        TextureRecipe::serialize(self, &mut serde_json::Serializer::new(&mut text))
            .map_err(S::Error::custom)?;
        let mut doc: Value = serde_json::from_slice(&text).map_err(S::Error::custom)?;
        unbind(&mut doc, &self.bound);
        doc.serialize(s)
    }
}

/// Swap every placeholder among the `nodes`' values for its param's default, and
/// return where each one was. A placeholder naming no declared param is refused.
fn bind(nodes: &mut [Value], params: &BTreeMap<String, ParamDecl>) -> Result<Vec<Bound>, String> {
    let mut bound = Vec::new();
    for node in nodes.iter_mut().filter_map(Value::as_object_mut) {
        let id = node
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let instance = node.get("op").and_then(Value::as_str) == Some("graph");
        for (key, value) in node
            .iter_mut()
            .filter(|(k, _)| !WIRING.contains(&k.as_str()))
        {
            // An instance's `params` map holds values for its graph (placeholders
            // among them, filled at expansion), never one placeholder itself.
            if instance && key == "params" {
                continue;
            }
            let Some(param) = placeholder(value).map(str::to_string) else {
                continue;
            };
            if instance && key == "path" {
                return Err(format!(
                    "node {id:?} (graph): path = {{ param = {param:?} }}: an instance's \
                     path is a file, never a param, since its graph is read before any \
                     param has a value"
                ));
            }
            *value = params
                .get(&param)
                .map(|d| d.default.clone())
                .ok_or_else(|| {
                    let declared: Vec<&str> = params.keys().map(String::as_str).collect();
                    format!(
                        "node {id:?}: {key} = {{ param = {param:?} }} names no param of the \
                     recipe; it declares [{}]",
                        declared.join(", ")
                    )
                })?;
            bound.push(Bound {
                node: id.clone(),
                key: key.clone(),
                param,
            });
        }
    }
    Ok(bound)
}

/// Write each of `bound`'s placeholders back into the serialized `doc`.
fn unbind(doc: &mut Value, bound: &[Bound]) {
    let Some(nodes) = doc.get_mut("nodes").and_then(Value::as_array_mut) else {
        return;
    };
    for b in bound {
        let node = nodes
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .find(|n| n.get("id").and_then(Value::as_str) == Some(&b.node));
        if let Some(node) = node {
            let placeholder = Map::from_iter([("param".to_string(), Value::from(b.param.clone()))]);
            node.insert(b.key.clone(), Value::Object(placeholder));
        }
    }
}

/// Refuse an input name a reference can't tell apart from a node or an output.
fn check_inputs(recipe: &TextureRecipe) -> Result<(), String> {
    for (i, name) in recipe.inputs.iter().enumerate() {
        if name.is_empty() || name.contains(['/', '.']) {
            return Err(format!(
                "input {name:?}: an input name is not empty and has no '/' or '.'"
            ));
        }
        if recipe.inputs[..i].contains(name) || recipe.nodes.iter().any(|n| &n.id == name) {
            return Err(format!(
                "input {name:?} is named twice (an input or a node id)"
            ));
        }
    }
    Ok(())
}
