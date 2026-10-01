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

mod catalog;
mod node;

pub use catalog::{Lit, OpInfo, OpParam, OPS};
pub use node::Node;

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
    /// `1 − |n|`, squared, summed over octaves: sharp crests (rock veins, scratches).
    Ridged,
    /// `|n|` summed over octaves: billowy creases (smoke, grime).
    Turbulence,
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
    /// Left→right linear ramp, 0→1 — the one generator that does **not** tile
    /// (a hard edge at the wrap); use it under a mask or a `mapping { tiling = false }`.
    Linear,
    /// Left→center→right triangle ramp, 0→1→0: the seamless linear ramp.
    LinearTiling,
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
    /// Distance to the second-nearest cell point (F2).
    F2,
    /// `F2 − F1`: ~0 on cell borders — cracks, dried mud, cobble outlines.
    Edges,
}

/// One stop in a [`ColorRamp`](OpKind::ColorRamp): position in `[0, 1]` and its color.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    // Periodic params (`scale`, `frequency`, `rows`/`cols`, `tiles`) are whole counts
    // of periods per tile: fractional values round at evaluation (#392).
    /// Gradient noise (grayscale, written to RGB, alpha 1). The fBM family (`fbm`,
    /// `ridged`, `turbulence`) sums `octaves`, each `lacunarity`× the previous
    /// frequency (rounded to a whole count, so every octave tiles) and `gain`× its
    /// amplitude; `perlin` is one octave.
    Noise {
        kind: NoiseKind,
        scale: f32,
        #[serde(default = "one_u32")]
        octaves: u32,
        #[serde(default = "two")]
        lacunarity: f32,
        #[serde(default = "half")]
        gain: f32,
    },
    /// Voronoi / Worley cellular pattern; `randomness` jitters each cell's point
    /// (0 = a regular grid, 1 = anywhere in its cell).
    Voronoi {
        scale: f32,
        #[serde(default)]
        output: VoronoiOutput,
        #[serde(default = "one_f32")]
        randomness: f32,
    },
    /// Linear or radial gradient ramp (grayscale).
    Gradient { kind: GradientKind },
    /// Bands or rings (grayscale), `frequency` cycles across the domain.
    Wave { kind: WaveKind, frequency: f32 },
    /// A brick / running-bond pattern; `rows`/`cols` bricks across the domain
    /// (`rows` rounds to an even count so the running bond closes at the wrap).
    Brick {
        rows: f32,
        cols: f32,
        #[serde(default = "default_mortar")]
        mortar: f32,
    },
    /// A 2-color checkerboard; `tiles` squares across each axis (rounded up to even).
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
    /// Scale / rotate (turns) / translate the *domain* of the single input, resampled
    /// bilinearly with wrap. With `tiling` (the default) `scale` rounds to whole
    /// repeats and `rotation` snaps to quarter turns, so the output tiles; `tiling =
    /// false` honours free values for decals and one-off maps, at the cost of a seam.
    Mapping {
        scale: [f32; 2],
        rotation: f32,
        translation: [f32; 2],
        #[serde(default = "yes")]
        tiling: bool,
    },
    /// Treat the input's red as a height field and bake a tangent-space normal map.
    /// `strength` scales the slope measured in **tile units** (height change across
    /// one whole tile), so it bakes the same normals at any resolution (#394).
    BumpToNormal { strength: f32 },
    /// Combine three single-channel inputs into one RGB image (alpha 1).
    CombineRgb,
    /// Pull one channel of the input out as grayscale. `channel` 0=R,1=G,2=B,3=A.
    SeparateRgb { channel: u8 },
    /// Domain warp (2 inputs): sample input A displaced by input B's R/G, centred
    /// on 0.5 and scaled by `strength` in **tile units**; wrapped, so it tiles.
    Warp { strength: f32 },

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
    /// A separable box blur (wraps at the edges). `radius` is a **fraction of the
    /// tile width** (`0.01` = 1%), so it looks the same at any resolution (#394).
    Blur { radius: f32 },
}

fn yes() -> bool {
    true
}

fn one_u32() -> u32 {
    1
}

fn one_f32() -> f32 {
    1.0
}

fn two() -> f32 {
    2.0
}

fn half() -> f32 {
    0.5
}

fn default_mortar() -> f32 {
    0.05
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
mod tests;
