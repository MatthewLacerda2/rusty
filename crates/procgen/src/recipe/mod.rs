//! crates/procgen/src/recipe/mod.rs — the texture recipe: a serde DAG of ops (the source of
//! truth).
//!
//! A [`TextureRecipe`] is a small **directed acyclic graph**: a flat list of
//! [`Node`]s, each with a stable `id`, an [`OpKind`] (the op + its params), and the
//! `id`s of the input node(s) it consumes. The op-runner ([`super::runner`])
//! evaluates the graph in dependency order to a linear-RGBA buffer, and the bake
//! ([`super::bake`]) encodes that buffer to a PNG.
//!
//! Recipe-as-truth: the document is plain serde data with **no GPU buffers and no
//! handles**, so it round-trips losslessly (save → load → re-bake is byte-identical)
//! and is the same shape whether it came from a `.json` on disk or a Lua table. This
//! is the shared spine the material (#271) and shader (#272) authoring legs reuse.
//!
//! Parsing is strict (#395): an unknown key anywhere — on the recipe, a node, or a
//! ramp stop — is an error naming it, so a typo (`scael = 8`) never bakes silently
//! with a default. [`Node`]'s check lives in `recipe/node.rs`.
//!
//! Every recipe can also be a **graph** another recipe instances (#1744): its
//! `params` and `inputs` are its interface, and a `{ param = "age" }` placeholder
//! in a node stands for a param's value. The placeholders are read, and written
//! back, by [`crate::graph`]'s serde hooks; the expansion is there too.

mod arity;
pub mod color;
mod extent;
mod kinds;
mod node;
mod op_info;
mod param;
#[cfg(test)]
mod sample;

/// The op-set, its params structs and the catalog: one file per op, in
/// [`crate::catalog`] (#1751).
pub use super::catalog::*;
pub use arity::Arity;
pub(crate) use arity::{arity, ONE, SOURCE};
pub(crate) use extent::power_of_two_sides;
pub use kinds::*;
pub use node::{Fault, Node, NodeError};
pub(crate) use op_info::{choice, op, opt, req, socket, SEED};
pub use op_info::{Lit, OpInfo, OpParam};
pub use param::{Link, Param};
#[cfg(test)]
pub(crate) use sample::sample_op;

/// The file a sampled `path` param names; tests that evaluate a sampled `image` op
/// hand procgen a source under this key. A test hook: the root's Lua enum-case test
/// fills a `path` param with it, so it is compiled in every build.
#[doc(hidden)]
pub const SAMPLE_PATH: &str = "textures/sample.png";

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::graph::{Bound, ParamDecl};

/// One stop in a [`ColorRamp`](OpKind::ColorRamp): position in `[0, 1]` and its color.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RampStop {
    pub pos: f32,
    #[serde(deserialize_with = "color::color")]
    pub color: [f32; 4],
}

/// The whole authored texture: the canvas extent (`resolution` squared, or `size`),
/// the RNG seed (folded into every stochastic op), and the flat node list. The last node in topological order
/// that nothing else consumes is the recipe's output (or the explicit `output` id).
///
/// Its serde impls are [`crate::graph`]'s: they read a node's
/// `{ param = … }` placeholders into [`TextureRecipe::bound`] and write them back
/// (`remote = "Self"` makes this derive an inherent function the writer calls; the
/// reader parses a field-for-field mirror, so a change here is made there too).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(remote = "Self")]
pub struct TextureRecipe {
    /// The side of a square bake. Ignored when `size` is set, so it may be left out
    /// then (it reads 0, and is not written).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub resolution: u32,
    /// A non-square bake's `[width, height]`, each a positive power of two, in
    /// place of `resolution` (#1493). [`TextureRecipe::extent`] reads the two.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "extent::power_of_two_sides"
    )]
    pub size: Option<[u32; 2]>,
    #[serde(default)]
    pub seed: u64,
    pub nodes: Vec<Node>,
    /// Explicit output node id. If absent, the runner uses the last node.
    #[serde(default)]
    pub output: Option<String>,
    /// Named outputs for a whole map set (#403): slot name → node id, all baked from
    /// one evaluation by `Texture.BakeSet`. Empty (the default) for a single-output
    /// recipe, and omitted from its JSON.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, String>,
    /// The params an instance of this recipe sets (#1744): name → type, default and
    /// range. Baked on its own, the recipe uses the defaults.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, ParamDecl>,
    /// The named inputs an instance wires, in order (#1744). Inner nodes name them
    /// like node ids. A recipe with inputs bakes only through an instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// Which node values are `{ param = … }` placeholders: parsed at the param's
    /// default, written back as the placeholder (#1744).
    #[serde(skip)]
    pub bound: Vec<Bound>,
}

impl TextureRecipe {
    /// A single-output recipe over `nodes`: seed 0, output the last node, no named
    /// outputs. Tests build recipes through this and the `with_*` setters below, so a
    /// new optional field touches only the code that uses it (#663). `resolution` is
    /// required, which is why there is no `Default`: a zero-sized recipe never bakes.
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

    /// Bake at `[width, height]` instead of `resolution` squared.
    pub fn with_size(mut self, size: [u32; 2]) -> Self {
        self.size = Some(size);
        self
    }

    /// Set the noise seed.
    /// Test seam for `tests/procgen/mod.rs`.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Name the output node explicitly instead of the last one.
    /// Test seam for `tests/procgen/mod.rs`.
    pub fn with_output(mut self, id: impl Into<String>) -> Self {
        self.output = Some(id.into());
        self
    }

    /// Every file path an `image` node reads (#1491), each once, in node order,
    /// with the id of the first node that names it: the files a caller decodes
    /// into [`crate::Sources`] before evaluating, and the node an error
    /// about one names.
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

    /// Serialize to pretty JSON — the on-disk recipe form.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

/// Serde's `skip_serializing_if` for a `resolution` a `size` recipe leaves out.
fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod socket_tests;
#[cfg(test)]
mod tests;
