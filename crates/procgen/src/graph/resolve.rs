use std::collections::BTreeMap;

use serde_json::Value;

use super::expand::Graphs;
use super::MAX_DEPTH;
use crate::recipe::{Node, OpKind, TextureRecipe};

pub(super) struct Scope<'a> {
    pub graph: &'a TextureRecipe,
    pub graphs: &'a Graphs,
    pub prefix: String,
    pub values: BTreeMap<String, Value>,
    pub wired: Vec<String>,
    pub fold: u32,
    pub chain: Vec<String>,
}

impl<'a> Scope<'a> {
    pub fn top(recipe: &'a TextureRecipe, graphs: &'a Graphs) -> Self {
        let values = recipe.params.iter();
        Scope {
            graph: recipe,
            graphs,
            prefix: String::new(),
            values: values
                .map(|(k, d)| (k.clone(), d.default.clone()))
                .collect(),
            wired: Vec::new(),
            fold: 0,
            chain: Vec::new(),
        }
    }

    pub fn id(&self, id: &str) -> String {
        format!("{}{id}", self.prefix)
    }

    /// What reference `r` (an input, a link's node, an output) names in the flat
    /// recipe: a declared input's wiring, an instance's output (`brick`,
    /// `brick.height`), or a node under its prefixed id. An id no node has is
    /// prefixed too, and the validation after expansion names it.
    pub fn resolve(&self, r: &str) -> Result<String, String> {
        if let Some(i) = self.graph.inputs.iter().position(|name| name == r) {
            return Ok(self.wired[i].clone());
        }
        let (head, tail) = split(r);
        match self
            .graph
            .nodes
            .iter()
            .find(|n| n.id == head)
            .map(|n| &n.op)
        {
            Some(OpKind::Graph(g)) => {
                output_of(self.graphs, &g.path, &self.id(head), tail, &self.chain)
            }
            _ => Ok(self.id(r)),
        }
    }

    pub fn bound_value(&self, node: &Node) -> Result<Value, String> {
        let mut value = serde_json::to_value(node).map_err(|e| self.in_file(e.to_string()))?;
        for b in self.graph.bound.iter().filter(|b| b.node == node.id) {
            if let (Some(map), Some(v)) = (value.as_object_mut(), self.values.get(&b.param)) {
                map.insert(b.key.clone(), v.clone());
            }
        }
        Ok(value)
    }

    pub fn graph_at(&self, path: &str, node: &str) -> Result<&'a TextureRecipe, String> {
        lookup(self.graphs, path).map_err(|e| self.in_file(format!("node {node:?}: {e}")))
    }

    pub fn in_file(&self, error: String) -> String {
        match self.chain.last() {
            Some(file) => format!("{error} (in {file})"),
            None => error,
        }
    }
}

/// `chain` with `path` entered, refused when `path` is already on it (a graph that
/// instances itself, however far down) or when it nests past [`MAX_DEPTH`].
pub(super) fn enter(chain: &[String], path: &str) -> Result<Vec<String>, String> {
    let mut next = chain.to_vec();
    next.push(path.to_string());
    if chain.iter().any(|p| p == path) {
        return Err(format!(
            "graphs instance each other in a cycle: {}",
            next.join(" → ")
        ));
    }
    if next.len() > MAX_DEPTH {
        return Err(format!(
            "graphs nest more than {MAX_DEPTH} deep: {}",
            next.join(" → ")
        ));
    }
    Ok(next)
}

fn output_of(
    graphs: &Graphs,
    path: &str,
    id: &str,
    tail: Option<&str>,
    chain: &[String],
) -> Result<String, String> {
    let chain = enter(chain, path)?;
    let graph = lookup(graphs, path)?;
    let target = match tail {
        None => graph
            .output
            .clone()
            .or_else(|| graph.nodes.last().map(|n| n.id.clone()))
            .ok_or_else(|| format!("graph {path} has no nodes"))?,
        Some(name) => graph.outputs.get(name).cloned().ok_or_else(|| {
            let has: Vec<&str> = graph.outputs.keys().map(String::as_str).collect();
            format!(
                "graph {path} has no output {name:?}; its outputs are [{}]",
                has.join(", ")
            )
        })?,
    };
    if graph.inputs.contains(&target) {
        return Err(format!(
            "graph {path}: output {target:?} names an input; an output is a node"
        ));
    }
    let (head, rest) = split(&target);
    match graph.nodes.iter().find(|n| n.id == head).map(|n| &n.op) {
        Some(OpKind::Graph(g)) => output_of(graphs, &g.path, &format!("{id}/{head}"), rest, &chain),
        _ => Ok(format!("{id}/{target}")),
    }
}

fn lookup<'g>(graphs: &'g Graphs, path: &str) -> Result<&'g TextureRecipe, String> {
    graphs
        .get(path)
        .ok_or_else(|| format!("graph {path} was not loaded (its loader reads every graph)"))
}

fn split(r: &str) -> (&str, Option<&str>) {
    match r.split_once('.') {
        Some((head, tail)) => (head, Some(tail)),
        None => (r, None),
    }
}
