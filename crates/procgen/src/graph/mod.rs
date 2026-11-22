//! An instance node:
//! ```lua
//! { id = "brick", op = "graph", path = "textures/brick.graph.json",
//!   params = { age = 0.5 }, seed = 3, inputs = { "mask" } }
//! ```
//! and downstream nodes read `brick` (the graph's `output`) or `brick.height` (its
//! named output `height`). Inside the graph a node's value may be the placeholder
//! `{ param = "age" }` ([`placeholder`]), which a parse reads at the param's default and
//! a save writes back ([`TextureRecipe::bound`]).

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

pub const EXTENSION: &str = ".graph.json";

pub const MAX_DEPTH: usize = 16;

pub const MAX_NODES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub path: String,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default)]
    pub seed: u32,
}

impl Row for Instance {
    const INFO: crate::recipe::OpInfo = op(
        "graph",
        "graph",
        arity(0, u8::MAX),
        &[req("path", "path"), opt("seed", "integer", Lit::Integer(0))],
    );
}

impl Op for Instance {
    fn run(&self, cx: &Cx) -> Image {
        Image::new(cx.size)
    }
}

pub fn placeholder(value: &Value) -> Option<&str> {
    let map = value.as_object()?;
    match (map.len(), map.get("param")) {
        (1, Some(Value::String(name))) => Some(name),
        _ => None,
    }
}

impl TextureRecipe {
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
mod guard_tests;
#[cfg(test)]
mod nest_tests;
#[cfg(test)]
mod refuse_tests;
#[cfg(test)]
mod shape_tests;
#[cfg(test)]
mod tests;
