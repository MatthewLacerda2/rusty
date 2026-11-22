//! Plain serde data, no GPU buffers or handles: save → load → re-bake is byte-identical.
//! Parsing is strict: an unknown key anywhere is an error naming it.
mod arity;
pub mod color;
mod extent;
mod kinds;
mod node;
mod op_info;
mod param;
mod sample;

pub use super::catalog::*;
pub use arity::Arity;
pub(crate) use arity::{arity, ONE, SOURCE};
pub(crate) use extent::power_of_two_sides;
pub use kinds::*;
pub use node::{Fault, Node, NodeError};
pub(crate) use op_info::{choice, op, opt, req, req_sampled, socket, SEED};
pub use op_info::{Lit, OpInfo, OpParam};
pub use param::{Link, Param};
#[doc(hidden)]
pub use sample::sample_required;
#[cfg(test)]
pub(crate) use sample::{reading_sample_op, sample_op};

#[doc(hidden)]
pub const SAMPLE_PATH: &str = "textures/sample.png";

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::graph::{Bound, ParamDecl};

/// A ramp stop: `pos` in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RampStop {
    pub pos: f32,
    #[serde(deserialize_with = "color::color")]
    pub color: [f32; 4],
}

/// [`crate::graph`] wraps this derive (`remote = "Self"`) and its reader parses a
/// field-for-field mirror, so a field change here is made there too.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(remote = "Self")]
pub struct TextureRecipe {
    /// Ignored when `size` is set (reads 0, not written).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub resolution: u32,
    /// `[width, height]`, each a positive power of two.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "extent::power_of_two_sides"
    )]
    pub size: Option<[u32; 2]>,
    #[serde(default)]
    pub seed: u64,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, ParamDecl>,
    /// A recipe with inputs bakes only through an instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    #[serde(skip)]
    pub bound: Vec<Bound>,
}

impl TextureRecipe {
    /// `resolution` is required, hence no `Default`: a zero-sized recipe never bakes.
    pub fn new(resolution: u32, nodes: Vec<Node>) -> Self {
        Self {
            resolution,
            size: None,
            seed: 0,
            nodes,
            output: None,
            outputs: BTreeMap::new(),
            params: BTreeMap::new(),
            inputs: Vec::new(),
            bound: Vec::new(),
        }
    }

    pub fn with_size(mut self, size: [u32; 2]) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_output(mut self, id: impl Into<String>) -> Self {
        self.output = Some(id.into());
        self
    }

    /// Each path once, in node order, with the first node naming it.
    pub fn image_paths(&self) -> Vec<(&str, &str)> {
        let mut paths: Vec<(&str, &str)> = Vec::new();
        for node in &self.nodes {
            if let OpKind::Image(p) = &node.op {
                if !paths.iter().any(|&(_, seen)| seen == p.path) {
                    paths.push((&node.id, &p.path));
                }
            }
        }
        paths
    }

    /// Each font once, in node order, with the first node naming it.
    pub fn font_paths(&self) -> Vec<(&str, &str)> {
        let mut paths: Vec<(&str, &str)> = Vec::new();
        for node in &self.nodes {
            if let OpKind::Text(p) = &node.op {
                if !p.font.is_empty() && !paths.iter().any(|&(_, seen)| seen == p.font) {
                    paths.push((&node.id, &p.font));
                }
            }
        }
        paths
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod socket_tests;
#[cfg(test)]
mod tests;
