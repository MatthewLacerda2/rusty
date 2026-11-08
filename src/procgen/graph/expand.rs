//! src/procgen/graph/expand.rs — a recipe and the graphs it instances, made one flat
//! recipe (#1744).
//!
//! Each instance's nodes are pasted in its place under the instance's id
//! (`brick/mortar`, nested `brick/wear/blur_2`), its placeholders take the values
//! the instance gives (or the defaults), its `inputs` read what the instance wires,
//! and its seed folds into each inner node's. Everything else is copied as written,
//! so a recipe without instances expands to itself and an instance at seed 0 bakes
//! exactly like its nodes pasted by hand.

use std::collections::BTreeMap;

use serde_json::Value;

use super::params::values;
use super::placeholder;
use super::resolve::{enter, Scope};
use crate::core::naming::variant_case::decode;
use crate::procgen::hash::instance_seed;
use crate::procgen::recipe::{Node, OpKind, TextureRecipe};
use crate::procgen::runner::{validate, RunError};

/// The graph documents a recipe's instances name, by the path they are named by.
pub type Graphs = BTreeMap<String, TextureRecipe>;

/// A recipe with every instance expanded, and which graph file each id prefix came
/// from, for the errors that name a node inside one.
#[derive(Debug)]
pub struct Expanded {
    pub recipe: TextureRecipe,
    /// `("brick/", "textures/brick.graph.json")`, outermost first.
    files: Vec<(String, String)>,
}

impl Expanded {
    /// The graph file node `id` came from; `None` for one of the recipe's own.
    pub fn file_of(&self, id: &str) -> Option<&str> {
        let deepest = self
            .files
            .iter()
            .filter(|(p, _)| id.starts_with(p.as_str()));
        deepest
            .max_by_key(|(p, _)| p.len())
            .map(|(_, f)| f.as_str())
    }

    /// [`validate`] the flat recipe; an error about a node from a graph names its
    /// file too.
    pub fn validate(&self) -> Result<(), String> {
        validate(&self.recipe).map_err(|e| match &e {
            RunError::Node(n) => match self.file_of(&n.node) {
                Some(file) => format!("{e} (in {file})"),
                None => e.to_string(),
            },
            _ => e.to_string(),
        })
    }
}

/// `recipe` with each `op = "graph"` node replaced by its graph's nodes, read from
/// `graphs` (keyed by the instance's `path`). A recipe that declares `inputs` is
/// refused: it bakes only through an instance.
pub fn expand(recipe: &TextureRecipe, graphs: &Graphs) -> Result<Expanded, String> {
    if !recipe.inputs.is_empty() {
        return Err(format!(
            "the recipe declares inputs [{}]: a graph with inputs bakes only through an \
             instance that wires them",
            recipe.inputs.join(", ")
        ));
    }
    let top = Scope::top(recipe, graphs);
    let mut out = Out::default();
    paste(&top, &mut out)?;
    let mut flat = recipe.clone();
    flat.nodes = out.nodes;
    flat.params.clear();
    flat.bound.clear();
    flat.output = match (&recipe.output, recipe.nodes.last()) {
        (Some(id), _) => Some(top.resolve(id)?),
        (None, Some(last)) if matches!(last.op, OpKind::Graph(_)) => Some(top.resolve(&last.id)?),
        (None, _) => None,
    };
    for node in flat.outputs.values_mut() {
        *node = top.resolve(node)?;
    }
    Ok(Expanded {
        recipe: flat,
        files: out.files,
    })
}

/// What an expansion has pasted so far.
#[derive(Default)]
struct Out {
    nodes: Vec<Node>,
    files: Vec<(String, String)>,
}

/// Paste `scope`'s nodes into `out`, each instance expanded in its place.
fn paste(scope: &Scope, out: &mut Out) -> Result<(), String> {
    for node in &scope.graph.nodes {
        let mut value = scope.bound_value(node)?;
        match &node.op {
            OpKind::Graph(_) => {
                fill_placeholders(scope, &mut value)?;
                let instance: Node = decode(value).map_err(|e| scope.in_file(e))?;
                paste_instance(scope, &instance, out)?;
            }
            _ => out.nodes.push(rewrite(scope, value)?),
        }
    }
    Ok(())
}

/// Paste the graph `node` instances, in `scope`.
fn paste_instance(scope: &Scope, node: &Node, out: &mut Out) -> Result<(), String> {
    let OpKind::Graph(instance) = &node.op else {
        return Ok(());
    };
    let chain = enter(&scope.chain, &instance.path)?;
    let graph = scope.graph_at(&instance.path, &node.id)?;
    if node.inputs.len() != graph.inputs.len() {
        return Err(scope.in_file(format!(
            "node {:?} (graph): {} takes {} inputs ({}), but {} given",
            scope.id(&node.id),
            instance.path,
            graph.inputs.len(),
            graph.inputs.join(", "),
            node.inputs.len()
        )));
    }
    let wired = node.inputs.iter().map(|r| scope.resolve(r));
    let child = Scope {
        graph,
        graphs: scope.graphs,
        prefix: format!("{}{}/", scope.prefix, node.id),
        values: values(&graph.params, &instance.params, &instance.path)?,
        wired: wired.collect::<Result<_, _>>()?,
        fold: instance_seed(instance.seed, scope.fold),
        chain,
    };
    out.files
        .push((child.prefix.clone(), instance.path.clone()));
    paste(&child, out)
}

/// An instance's `params` placeholders, swapped for `scope`'s values.
fn fill_placeholders(scope: &Scope, node: &mut Value) -> Result<(), String> {
    let Some(params) = node.get_mut("params").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    for value in params.values_mut() {
        if let Some(name) = placeholder(value) {
            *value = scope.values.get(name).cloned().ok_or_else(|| {
                scope.in_file(format!(
                    "{{ param = {name:?} }} names no param of the graph"
                ))
            })?;
        }
    }
    Ok(())
}

/// A typed node's value, its id prefixed, its inputs and links resolved and its
/// seed folded, parsed back strictly.
fn rewrite(scope: &Scope, mut value: Value) -> Result<Node, String> {
    let Some(map) = value.as_object_mut() else {
        return Err(scope.in_file("a node is not an object".into()));
    };
    if let Some(Value::String(id)) = map.get_mut("id") {
        *id = scope.id(id);
    }
    if let Some(Value::Array(inputs)) = map.get_mut("inputs") {
        for input in inputs {
            if let Value::String(r) = input {
                *r = scope.resolve(r)?;
            }
        }
    }
    for (key, v) in map.iter_mut() {
        match (key.as_str(), &mut *v) {
            ("seed", Value::Number(n)) if scope.fold != 0 => {
                let seed = n.as_u64().and_then(|s| u32::try_from(s).ok()).unwrap_or(0);
                *v = Value::from(instance_seed(seed, scope.fold));
            }
            (_, Value::Object(link)) => {
                if let Some(Value::String(r)) = link.get_mut("node") {
                    *r = scope.resolve(r)?;
                }
            }
            _ => {}
        }
    }
    decode(value).map_err(|e| scope.in_file(e))
}
