//! src/procgen/recipe.rs — the texture recipe: a serde DAG of ops (the source of
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

use serde::{Deserialize, Serialize};

/// A blend mode for the [`OpKind::Mix`] op — the common Blender / Photoshop set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    /// Linear interpolate a→b by the factor.
    Mix,
    Add,
    Multiply,
    Screen,
    Overlay,
    Subtract,
    Difference,
}

/// A scalar math operation for [`OpKind::Math`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Min,
    Max,
    Abs,
    Fract,
    Sqrt,
}

/// Which kind of noise [`OpKind::Noise`] produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseKind {
    /// A single octave of gradient (Perlin) noise.
    Perlin,
    /// Fractional Brownian motion: several Perlin octaves summed.
    Fbm,
}

/// Which kind of wave [`OpKind::Wave`] produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveKind {
    /// Parallel bands across the domain.
    Bands,
    /// Concentric rings from the domain center.
    Rings,
}

/// Which kind of gradient [`OpKind::Gradient`] produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientKind {
    /// Left→right linear ramp.
    Linear,
    /// Center→edge radial ramp.
    Radial,
}

/// What the Voronoi op outputs per pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoronoiOutput {
    /// Distance to the nearest cell point (F1) — classic cellular.
    #[default]
    Distance,
    /// A flat random color per cell.
    Cells,
}

/// One stop in a [`ColorRamp`](OpKind::ColorRamp): position in `[0, 1]` and its color.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RampStop {
    pub pos: f32,
    pub color: [f32; 4],
}

/// The curated op-set ("Blender-lite"). Each variant carries its own params; the
/// node's `inputs` supply the upstream buffers it consumes. Generators take zero
/// inputs; most color/math ops take one; [`OpKind::Mix`] and [`OpKind::CombineRgb`]
/// take more.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OpKind {
    // ---- Generators (no inputs) ----
    /// A flat color fill.
    Constant { color: [f32; 4] },
    /// Perlin / fBM gradient noise (grayscale, written to RGB, alpha 1).
    Noise {
        kind: NoiseKind,
        scale: f32,
        #[serde(default = "one_u32")]
        octaves: u32,
    },
    /// Voronoi / Worley cellular pattern.
    Voronoi {
        scale: f32,
        #[serde(default)]
        output: VoronoiOutput,
    },
    /// Linear or radial gradient ramp (grayscale).
    Gradient { kind: GradientKind },
    /// Bands or rings (grayscale), `frequency` cycles across the domain.
    Wave { kind: WaveKind, frequency: f32 },
    /// A brick / running-bond pattern; `rows`/`cols` bricks across the domain.
    Brick {
        rows: f32,
        cols: f32,
        #[serde(default = "default_mortar")]
        mortar: f32,
    },
    /// A 2-color checkerboard; `tiles` squares across each axis.
    Checker {
        tiles: u32,
        color_a: [f32; 4],
        color_b: [f32; 4],
    },
    /// Per-pixel uniform white noise (grayscale), seeded by the recipe seed.
    WhiteNoise,

    // ---- Color (1 input, or 2 for Mix) ----
    /// Map the input's red channel through a gradient of stops.
    ColorRamp { stops: Vec<RampStop> },
    /// Blend two inputs by `factor` under a [`BlendMode`].
    Mix { mode: BlendMode, factor: f32 },
    /// Invert RGB (`1 - c`), alpha untouched.
    Invert,
    /// Brightness offset then contrast about 0.5.
    BrightContrast { bright: f32, contrast: f32 },
    /// Hue rotate (turns), saturation and value scale.
    HueSatValue { hue: f32, sat: f32, value: f32 },
    /// Per-channel gamma (`c^gamma`).
    Gamma { gamma: f32 },

    // ---- Vector / normal ----
    /// Scale / rotate / translate the *domain* of the single input (resampled with
    /// wrap, so it stays tiling).
    Mapping {
        scale: [f32; 2],
        rotation: f32,
        translation: [f32; 2],
    },
    /// Treat the input's red as a height field and bake a tangent-space normal map.
    BumpToNormal { strength: f32 },
    /// Combine three single-channel inputs into one RGB image (alpha 1).
    CombineRgb,
    /// Pull one channel of the input out as grayscale. `channel` 0=R,1=G,2=B,3=A.
    SeparateRgb { channel: u8 },

    // ---- Converter / math ----
    /// Per-channel scalar math, RHS is the constant `value`.
    Math { func: MathOp, value: f32 },
    /// Remap each channel from `[from_min, from_max]` to `[to_min, to_max]`.
    MapRange {
        from_min: f32,
        from_max: f32,
        to_min: f32,
        to_max: f32,
    },
    /// Clamp each channel into `[min, max]`.
    Clamp { min: f32, max: f32 },
    /// Collapse RGB to luminance (Rec. 709), written to all of RGB.
    RgbToBw,

    // ---- Filter ----
    /// A small separable box blur of `radius` pixels (wraps at the edges).
    Blur { radius: u32 },
}

fn one_u32() -> u32 {
    1
}

fn default_mortar() -> f32 {
    0.05
}

/// One node in the recipe DAG: a stable `id`, the op to run, and the `id`s of the
/// input nodes feeding it (in order).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub op: OpKind,
    #[serde(default)]
    pub inputs: Vec<String>,
}

/// The whole authored texture: the canvas resolution, the RNG seed (folded into
/// every stochastic op), and the flat node list. The last node in topological order
/// that nothing else consumes is the recipe's output (or the explicit `output` id).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextureRecipe {
    pub resolution: u32,
    #[serde(default)]
    pub seed: u64,
    pub nodes: Vec<Node>,
    /// Explicit output node id. If absent, the runner uses the last node.
    #[serde(default)]
    pub output: Option<String>,
}

impl TextureRecipe {
    /// Serialize to pretty JSON — the on-disk recipe form.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Parse a recipe from its JSON form.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe() -> TextureRecipe {
        TextureRecipe {
            resolution: 64,
            seed: 7,
            nodes: vec![
                Node {
                    id: "n0".into(),
                    op: OpKind::Checker {
                        tiles: 4,
                        color_a: [0.0, 0.0, 0.0, 1.0],
                        color_b: [1.0, 1.0, 1.0, 1.0],
                    },
                    inputs: vec![],
                },
                Node {
                    id: "n1".into(),
                    op: OpKind::Invert,
                    inputs: vec!["n0".into()],
                },
            ],
            output: Some("n1".into()),
        }
    }

    #[test]
    fn recipe_round_trips_through_json_unchanged() {
        let r = sample_recipe();
        let json = r.to_json().expect("serialize");
        let back = TextureRecipe::from_json(&json).expect("deserialize");
        assert_eq!(r, back);
    }

    #[test]
    fn op_tag_is_snake_case() {
        let json = serde_json::to_string(&OpKind::WhiteNoise).unwrap();
        assert!(json.contains("\"white_noise\""), "got {json}");
    }

    #[test]
    fn voronoi_output_defaults_to_distance_when_absent() {
        let json = r#"{ "id": "v", "op": "voronoi", "scale": 8.0 }"#;
        let node: Node = serde_json::from_str(json).expect("parse");
        match node.op {
            OpKind::Voronoi { output, .. } => assert_eq!(output, VoronoiOutput::Distance),
            other => panic!("wrong op: {other:?}"),
        }
    }
}
