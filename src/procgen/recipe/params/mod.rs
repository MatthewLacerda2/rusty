//! src/procgen/recipe/params.rs — each op's params as its own struct (#1485).
//!
//! [`super::OpKind`] is a tagged enum of newtype variants, `Noise(Noise)`, so the
//! dispatcher matches every op on one line and hands the op its params whole. The
//! JSON form is unchanged: serde's internal `op` tag flattens a newtype struct's
//! fields beside it, exactly as it did a struct variant's. The generators' params
//! are in `generators.rs`; the serde default functions here serve both files.
//!
//! A [`Param`] field is **socketable** (#1494): it takes a number or a link to
//! another node. The catalog's `socket` flag lists the same fields
//! (`catalog_tests.rs` holds the two equal). Those structs aren't `Copy`, since a
//! link names its node.

use serde::{Deserialize, Serialize};

use super::{BlendMode, BlurKind, MathOp, Param, RampInterpolation, RampStop};

mod generators;

pub use generators::*;

// ---- Color ----

/// Map the input's red channel through a gradient of stops, filled between them by
/// `interpolation` (Blender's ColorRamp).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorRamp {
    pub stops: Vec<RampStop>,
    #[serde(default)]
    pub interpolation: RampInterpolation,
}

/// Blend inputs `a`, `b` by `factor` under a [`BlendMode`]; an optional third input
/// is a per-pixel mask whose red multiplies `factor` (#405).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    pub mode: BlendMode,
    #[serde(default = "one_f32")]
    pub factor: f32,
}

/// Brightness offset then contrast about 0.5.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrightContrast {
    pub bright: Param,
    pub contrast: Param,
}

/// Hue rotate (turns), saturation and value scale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HueSatValue {
    pub hue: Param,
    pub sat: Param,
    pub value: Param,
}

/// Per-channel gamma (`c^gamma`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gamma {
    pub gamma: Param,
}

// ---- Vector / normal ----

/// Scale / rotate (turns) / translate the *domain* of the single input, resampled
/// bilinearly with wrap. With `tiling` (the default) `scale` rounds to whole repeats
/// and `rotation` snaps to quarter turns, so the output tiles; `tiling = false`
/// honours free values for decals and one-off maps, at the cost of a seam. Each
/// transform left out is the identity, as in Blender's Mapping node (#1575).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    #[serde(default = "one_vec2")]
    pub scale: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub translation: [f32; 2],
    #[serde(default = "yes")]
    pub tiling: bool,
}

/// Treat the input's red as a height field and bake a tangent-space normal map.
/// `strength` scales the slope measured in **tile units** (height change across one
/// whole tile), so it bakes the same normals at any resolution (#394).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BumpToNormal {
    pub strength: Param,
}

/// Pull one channel of the input out as grayscale. `channel` 0=R,1=G,2=B,3=A.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeparateRgb {
    pub channel: u8,
}

/// Scatter: stamp the input once per cell of a `count`×`count` grid, each cell
/// seeded with an offset (`jitter`, in cells), a rotation (`rotation_jitter`, in
/// turns) and a scale (`scale_jitter`); overlaps keep the brighter stamp. Always
/// tiles. A linked jitter is read once per cell, at its centre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub count: f32,
    #[serde(default)]
    pub jitter: Param,
    #[serde(default)]
    pub rotation_jitter: Param,
    #[serde(default)]
    pub scale_jitter: Param,
    #[serde(default)]
    pub seed: u32,
}

/// Reoriented Normal Mapping (#1492): lay the second input's normal map (detail)
/// over the first's (base). `strength` scales the detail's tilt first; 0 leaves the
/// base.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalBlend {
    #[serde(default = "one_param")]
    pub strength: Param,
}

/// Domain warp: sample input A displaced by input B's R/G, centred on 0.5 and scaled
/// by `strength` in **tile units**; wrapped, so it tiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    pub strength: Param,
}

// ---- Converter / math ----

/// Per-channel math. The right-hand side is the constant `value`, or, when a second
/// input is wired, that input's matching channel per pixel (`value` is then ignored,
/// Blender's Math with both sockets linked). `value` defaults to 0.5, Blender's
/// socket default, so a wired second input needs no dummy `value`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Math {
    pub func: MathOp,
    #[serde(default = "half")]
    pub value: f32,
}

/// Remap each channel from `[from_min, from_max]` to `[to_min, to_max]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapRange {
    pub from_min: f32,
    pub from_max: f32,
    pub to_min: f32,
    pub to_max: f32,
}

/// Clamp each channel into `[min, max]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clamp {
    pub min: f32,
    pub max: f32,
}

// ---- Filter ----

/// A separable blur (wraps at the edges), Gaussian by default. `radius` is the
/// kernel's reach as a **fraction of the tile width** (`0.01` = 1%), so it looks the
/// same at any resolution (#394). A linked radius varies per pixel (#1494).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blur {
    pub radius: Param,
    #[serde(default)]
    pub kind: BlurKind,
}

/// Height → cavity mask: `1 − max(blur(h) − h, 0)` with a `radius` Gaussian blur
/// (tile fraction, as `blur`). White on flat and raised ground, dark in crevices (#408).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cavity {
    pub radius: f32,
}

/// Height → curvature: `0.5 + strength × (h − mean of h on a ring of `radius`)` (tile
/// units). Above 0.5 convex (edge wear), below concave (grime) (#408).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curvature {
    pub radius: f32,
    #[serde(default = "one_param")]
    pub strength: Param,
}

/// Mask → bevel (#1492): red > 0.5 is inside; each inside pixel reads its distance
/// to the mask's edge over `radius` (tile units), clamped to 1. Outside reads 0.
/// Wraps, so a tiling mask gives a tiling bevel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Distance {
    pub radius: Param,
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

fn one_vec2() -> [f32; 2] {
    [1.0, 1.0]
}

fn one_param() -> Param {
    Param::Num(1.0)
}

fn half_param() -> Param {
    Param::Num(0.5)
}

fn two() -> f32 {
    2.0
}

fn half() -> f32 {
    0.5
}

fn quarter() -> f32 {
    0.25
}

fn default_mortar() -> f32 {
    0.05
}
