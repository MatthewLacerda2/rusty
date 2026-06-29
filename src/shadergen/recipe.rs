//! src/shadergen/recipe.rs — the shader recipe: a serde document selecting a base
//! pass kind and an ordered list of building blocks (the source of truth).
//!
//! A [`ShaderRecipe`] names a base [`PassKind`] (the contract the output honours)
//! and a flat, ordered list of [`BlockSel`] — each a block `id` from the curated
//! library plus its parameters. The [`assemble`](super::assemble) step templates
//! the chosen blocks into a complete `.wgsl` module, [`validate`](super::validate)
//! composes it through `naga_oil`, and [`bake`](super::bake) writes it.
//!
//! Recipe-as-truth: the document is plain serde data with **no WGSL text and no
//! GPU handles** — just block ids + params — so it round-trips losslessly (save →
//! load → re-bake assembles byte-identical WGSL) and is the same shape whether it
//! came from a `.json` on disk or a Lua table. Block ordering in `blocks` is the
//! ordering the assembler applies them, so assembly is deterministic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The base pass a recipe targets — selects the contract the assembled module
/// must honour (entry points, bind groups, vertex/fragment IO) and which block
/// catalog is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassKind {
    /// Forward/surface pass: the standard `vs_main` vertex stage + lighting from
    /// `shader.wgsl`, with the fragment look varied by the chosen surface blocks.
    /// Consumes `LightingUniforms`, `EntityUniforms`, and the group(2) material
    /// texture bindings.
    Surface,
    /// Postfx pass: a fullscreen-triangle fragment program over the HDR target,
    /// the most self-contained pass. The chosen postfx blocks transform the
    /// sampled scene color before output.
    Postfx,
}

impl PassKind {
    /// The lowercase tag used in error messages and the block catalog lookup.
    pub fn tag(self) -> &'static str {
        match self {
            PassKind::Surface => "surface",
            PassKind::Postfx => "postfx",
        }
    }
}

/// A single parameter value a block consumes: a scalar or a small float vector
/// (a color or a 2-vector). Kept deliberately narrow — shader block params are
/// tuning knobs, not arbitrary data — so the assembler can emit each as a WGSL
/// literal with no ambiguity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    /// A single `f32` scalar (e.g. a strength, a threshold, a speed).
    Scalar(f32),
    /// A small float vector (e.g. an RGB tint `[r, g, b]` or a 2-vec).
    Vector(Vec<f32>),
}

/// One selected building block: its `id` in the curated library and the params
/// it is configured with. Unsupplied params fall back to the block's declared
/// defaults at assemble time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockSel {
    /// The block id, e.g. `"toon_ramp"` (surface) or `"vignette"` (postfx). Must
    /// exist in the catalog for the recipe's `pass`.
    pub id: String,
    /// Block parameters by name. A `BTreeMap` so serialization + the assembled
    /// WGSL are deterministic regardless of insertion order.
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
}

/// A complete shader recipe: the base pass kind, a stable module `name` (the
/// baked file becomes `<name>.wgsl`), and the ordered block list. The assembler
/// produces a contract-conformant module; nothing here carries WGSL text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderRecipe {
    /// The base pass the output conforms to.
    pub pass: PassKind,
    /// The module name the bake registers it by (`<name>.wgsl`). Used in the
    /// assembled module's header comment so a baked file is self-describing.
    pub name: String,
    /// The blocks to apply, in order.
    #[serde(default)]
    pub blocks: Vec<BlockSel>,
}

impl ShaderRecipe {
    /// Serialize to pretty JSON — the on-disk recipe form.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Parse a recipe from its JSON form.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

impl ParamValue {
    /// Read this value as a scalar, or `default` if it is a vector (a caller
    /// asking for a scalar from a vector param is a recipe mistake the assembler
    /// degrades over rather than failing the whole bake).
    pub fn scalar(&self, default: f32) -> f32 {
        match self {
            ParamValue::Scalar(s) => *s,
            ParamValue::Vector(_) => default,
        }
    }

    /// Read this value as an `n`-component vector, padding with `0.0` (or reading
    /// a scalar as `[s; n]`) so the assembler always has exactly `n` lanes to emit.
    pub fn vector(&self, n: usize) -> Vec<f32> {
        let mut out = match self {
            ParamValue::Scalar(s) => vec![*s; n],
            ParamValue::Vector(v) => v.clone(),
        };
        out.resize(n, 0.0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe() -> ShaderRecipe {
        let mut params = BTreeMap::new();
        params.insert("steps".to_string(), ParamValue::Scalar(4.0));
        ShaderRecipe {
            pass: PassKind::Surface,
            name: "toon".into(),
            blocks: vec![BlockSel {
                id: "toon_ramp".into(),
                params,
            }],
        }
    }

    #[test]
    fn recipe_round_trips_through_json_unchanged() {
        let r = sample_recipe();
        let json = r.to_json().expect("serialize");
        let back = ShaderRecipe::from_json(&json).expect("deserialize");
        assert_eq!(r, back);
    }

    #[test]
    fn pass_tag_is_snake_case() {
        let json = serde_json::to_string(&PassKind::Postfx).unwrap();
        assert_eq!(json, "\"postfx\"");
    }

    #[test]
    fn param_scalar_and_vector_coerce() {
        assert_eq!(ParamValue::Scalar(2.0).scalar(0.0), 2.0);
        assert_eq!(ParamValue::Vector(vec![1.0]).scalar(9.0), 9.0);
        assert_eq!(ParamValue::Scalar(0.5).vector(3), vec![0.5, 0.5, 0.5]);
        assert_eq!(
            ParamValue::Vector(vec![1.0, 2.0]).vector(3),
            vec![1.0, 2.0, 0.0]
        );
    }

    #[test]
    fn untagged_param_parses_scalar_or_array() {
        let s: ParamValue = serde_json::from_str("0.5").unwrap();
        assert_eq!(s, ParamValue::Scalar(0.5));
        let v: ParamValue = serde_json::from_str("[1.0, 0.0, 0.0]").unwrap();
        assert_eq!(v, ParamValue::Vector(vec![1.0, 0.0, 0.0]));
    }
}
