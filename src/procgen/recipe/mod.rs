//! src/procgen/recipe/mod.rs — the texture recipe: a serde DAG of ops (the source of
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

mod arity;
mod catalog;
mod kinds;
mod node;
pub mod params;
#[cfg(test)]
mod sample;
mod tag;

pub use arity::Arity;
pub use catalog::{Lit, OpInfo, OpParam, OPS};
pub use kinds::*;
pub use node::{ArityError, Node};
pub use params::*;
#[cfg(test)]
pub(crate) use sample::sample_op;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One stop in a [`ColorRamp`](OpKind::ColorRamp): position in `[0, 1]` and its color.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RampStop {
    pub pos: f32,
    pub color: [f32; 4],
}

/// The curated op-set ("Blender-lite"). Each variant carries its params as one
/// struct ([`params`]); the node's `inputs` supply the upstream buffers it consumes,
/// as many as the op's [`Arity`] in [`OPS`] allows (#1485).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OpKind {
    // ---- Generators (no inputs) ----
    Constant(Constant),
    Noise(Noise),
    Voronoi(Voronoi),
    Gradient(Gradient),
    Wave(Wave),
    Brick(Brick),
    Shape(Shape),
    Checker(Checker),
    /// Per-pixel uniform white noise (grayscale), seeded by the recipe seed.
    WhiteNoise,

    // ---- Color ----
    ColorRamp(ColorRamp),
    Mix(Mix),
    /// Invert RGB (`1 - c`), alpha untouched.
    Invert,
    BrightContrast(BrightContrast),
    HueSatValue(HueSatValue),
    Gamma(Gamma),

    // ---- Vector / normal ----
    Mapping(Mapping),
    BumpToNormal(BumpToNormal),
    /// Combine up to three single-channel inputs into one RGB image (alpha 1); a
    /// channel without an input reads 0.
    CombineRgb,
    SeparateRgb(SeparateRgb),
    Tile(Tile),
    Warp(Warp),

    // ---- Converter / math ----
    Math(Math),
    MapRange(MapRange),
    Clamp(Clamp),
    /// Collapse RGB to luminance (Rec. 709), written to all of RGB.
    RgbToBw,

    // ---- Filter ----
    Blur(Blur),
    Cavity(Cavity),
    Curvature(Curvature),
}

/// The whole authored texture: the canvas resolution, the RNG seed (folded into
/// every stochastic op), and the flat node list. The last node in topological order
/// that nothing else consumes is the recipe's output (or the explicit `output` id).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextureRecipe {
    pub resolution: u32,
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
}

impl TextureRecipe {
    /// A single-output recipe over `nodes`: seed 0, output the last node, no named
    /// outputs. Tests build recipes through this and the `with_*` setters below, so a
    /// new optional field touches only the code that uses it (#663). `resolution` is
    /// required, which is why there is no `Default`: a zero-sized recipe never bakes.
    pub fn new(resolution: u32, nodes: Vec<Node>) -> Self {
        Self {
            resolution,
            seed: 0,
            nodes,
            output: None,
            outputs: BTreeMap::new(),
        }
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

    /// Serialize to pretty JSON — the on-disk recipe form.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests;
