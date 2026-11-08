//! src/procgen/graph — a recipe used as one node of another (#1744), as Substance
//! Designer's graph instances and Blender's node groups.
//!
//! Any recipe can be a graph: its `params` (name → type, default, range) and its
//! `inputs` (ordered names its nodes read like node ids) are its interface, and its
//! `output` / `outputs` are what it gives back. Saved as `<name>.graph.json`, it is
//! instanced by a node
//!
//! ```lua
//! { id = "brick", op = "graph", path = "textures/brick.graph.json",
//!   params = { age = 0.5 }, seed = 3, inputs = { "mask" } }
//! ```
//!
//! and downstream nodes read `brick` (the graph's `output`) or `brick.height` (its
//! named output `height`). Inside the graph a node's value may be the placeholder
//! `{ param = "age" }` ([`params`]), which a parse reads at the param's default and
//! a save writes back ([`TextureRecipe::bound`]).
//!
//! What a recipe stores is the instance, never its expansion. [`expand`] turns a
//! recipe and the graph documents its caller read into one flat recipe: inner ids
//! become `brick/<inner id>`, references are rewritten, placeholders take the
//! instance's values and the instance's `seed` folds into every inner seed. The
//! runner only ever sees that flat graph, and refuses an instance left in one. This
//! module reads no files: the loader beside the `image` sources does
//! (`scene::authoring::material::sources`).

mod bind;
mod expand;
mod params;
mod resolve;

pub use bind::Bound;
pub use expand::{expand, Expanded, Graphs};
pub use params::{ParamDecl, ParamType};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::catalog::{Cx, Op, Row};
use super::image_buf::Image;
use super::recipe::{arity, op, opt, req, Lit, OpKind, TextureRecipe};

/// The extension that makes a file a graph: [`expand`]'s loader refuses to instance
/// any other, and `Texture.Ops()` lists the project's files that carry it.
pub const EXTENSION: &str = ".graph.json";

/// How many graphs deep instances may nest.
pub const MAX_DEPTH: usize = 16;

/// An `op = "graph"` node's own fields: the graph file, the values it gives the
/// graph's params, and the seed folded into the graph's nodes. Its `inputs` are the
/// node's, wired to the graph's `inputs` in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    /// The `.graph.json` file, from the project root.
    pub path: String,
    /// Param name → value; a value may be a `{ param = … }` placeholder of the
    /// recipe the instance sits in. Unset params take the graph's defaults.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    /// Folded into every inner node's seed; 0 leaves them as written.
    #[serde(default)]
    pub seed: u32,
}

impl Row for Instance {
    /// Not in [`super::recipe::OPS`]: `Texture.Ops()` lists each project graph
    /// with its own inputs and params instead.
    const INFO: crate::procgen::recipe::OpInfo = op(
        "graph",
        "graph",
        arity(0, u8::MAX),
        &[req("path", "path"), opt("seed", "integer", Lit::Integer(0))],
    );
}

impl Op for Instance {
    /// Never reached: the runner refuses an unexpanded instance before anything
    /// runs. A blank image keeps the sim panic-free all the same.
    fn run(&self, cx: &Cx) -> Image {
        Image::new(cx.size)
    }
}

/// The param `name` when `value` is the placeholder `{ param = "name" }`.
pub fn placeholder(value: &Value) -> Option<&str> {
    let map = value.as_object()?;
    match (map.len(), map.get("param")) {
        (1, Some(Value::String(name))) => Some(name),
        _ => None,
    }
}

impl TextureRecipe {
    /// Every graph file an `op = "graph"` node of this recipe names, each once, with
    /// the first node that names it: the files its loader reads before [`expand`].
    pub fn graph_paths(&self) -> Vec<(&str, &str)> {
        let mut paths: Vec<(&str, &str)> = Vec::new();
        for node in &self.nodes {
            if let OpKind::Graph(g) = &node.op {
                if !paths.iter().any(|&(_, seen)| seen == g.path) {
                    paths.push((&node.id, &g.path));
                }
            }
        }
        paths
    }
}

#[cfg(test)]
mod nest_tests;
#[cfg(test)]
mod refuse_tests;
#[cfg(test)]
mod tests;
