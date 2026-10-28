//! src/procgen/recipe/params.rs — each op's params as its own struct (#1485).
//!
//! [`super::OpKind`] is a tagged enum of newtype variants, `Noise(Noise)`, so the
//! dispatcher matches every op on one line and hands the op its params whole. The
//! JSON form is unchanged: serde's internal `op` tag flattens a newtype struct's
//! fields beside it, exactly as it did a struct variant's.

use serde::{Deserialize, Serialize};

use super::color;
use super::{
    BlendMode, BrickOutput, GradientKind, MathOp, NoiseKind, RampStop, ShapeKind, VoronoiOutput,
    WaveKind,
};

// ---- Generators (no inputs) ----
// Periodic params (`scale`, `frequency`, `rows`/`cols`, `tiles`) are whole counts of
// periods per tile: fractional values round at evaluation (#392). A stochastic op's
// `seed` (default 0) is folded into the recipe seed (`hash::node_seed`), so two
// otherwise identical nodes differ by seed alone (#1487); 0 keeps the recipe seed.

/// A flat color fill.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constant {
    #[serde(deserialize_with = "color::color")]
    pub color: [f32; 4],
}

/// Gradient noise (grayscale, written to RGB, alpha 1). The fBM family (`fbm`,
/// `ridged`, `turbulence`) sums `octaves`, each `lacunarity`× the previous frequency
/// (rounded to a whole count, so every octave tiles) and `gain`× its amplitude;
/// `perlin` is one octave.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Noise {
    pub kind: NoiseKind,
    pub scale: f32,
    #[serde(default = "one_u32")]
    pub octaves: u32,
    #[serde(default = "two")]
    pub lacunarity: f32,
    #[serde(default = "half")]
    pub gain: f32,
    #[serde(default)]
    pub seed: u32,
}

/// Voronoi / Worley cellular pattern; `randomness` jitters each cell's point (0 = a
/// regular grid, 1 = anywhere in its cell).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Voronoi {
    pub scale: f32,
    #[serde(default)]
    pub output: VoronoiOutput,
    #[serde(default = "one_f32")]
    pub randomness: f32,
    #[serde(default)]
    pub seed: u32,
}

/// Linear or radial gradient ramp (grayscale).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub kind: GradientKind,
}

/// Bands or rings (grayscale), `frequency` cycles across the domain.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wave {
    pub kind: WaveKind,
    pub frequency: f32,
}

/// A brick / running-bond pattern; `rows`/`cols` bricks across the domain (`rows`
/// rounds to an even count so the running bond closes at the wrap). `output` picks
/// the face mask, a seeded value per brick, or a bevel ramp.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brick {
    pub rows: f32,
    pub cols: f32,
    #[serde(default = "default_mortar")]
    pub mortar: f32,
    #[serde(default)]
    pub output: BrickOutput,
    #[serde(default)]
    pub seed: u32,
}

/// One centred shape per tile as a mask (1 inside, 0 outside). `size` is its
/// width/height in tile units; `roundness` (0–1) rounds a `rounded_rect`'s corners;
/// `softness` (tile units) ramps the edge inward, a bevel profile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub size: [f32; 2],
    #[serde(default = "quarter")]
    pub roundness: f32,
    #[serde(default)]
    pub softness: f32,
}

/// A 2-color checkerboard; `tiles` squares across each axis (rounded up to even).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checker {
    pub tiles: u32,
    #[serde(deserialize_with = "color::color_a")]
    pub color_a: [f32; 4],
    #[serde(deserialize_with = "color::color_b")]
    pub color_b: [f32; 4],
}

/// Per-pixel uniform white noise (grayscale).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WhiteNoise {
    #[serde(default)]
    pub seed: u32,
}

// ---- Color ----

/// Map the input's red channel through a gradient of stops.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorRamp {
    pub stops: Vec<RampStop>,
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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrightContrast {
    pub bright: f32,
    pub contrast: f32,
}

/// Hue rotate (turns), saturation and value scale.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HueSatValue {
    pub hue: f32,
    pub sat: f32,
    pub value: f32,
}

/// Per-channel gamma (`c^gamma`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gamma {
    pub gamma: f32,
}

// ---- Vector / normal ----

/// Scale / rotate (turns) / translate the *domain* of the single input, resampled
/// bilinearly with wrap. With `tiling` (the default) `scale` rounds to whole repeats
/// and `rotation` snaps to quarter turns, so the output tiles; `tiling = false`
/// honours free values for decals and one-off maps, at the cost of a seam.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    pub scale: [f32; 2],
    pub rotation: f32,
    pub translation: [f32; 2],
    #[serde(default = "yes")]
    pub tiling: bool,
}

/// Treat the input's red as a height field and bake a tangent-space normal map.
/// `strength` scales the slope measured in **tile units** (height change across one
/// whole tile), so it bakes the same normals at any resolution (#394).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BumpToNormal {
    pub strength: f32,
}

/// Pull one channel of the input out as grayscale. `channel` 0=R,1=G,2=B,3=A.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeparateRgb {
    pub channel: u8,
}

/// Scatter: stamp the input once per cell of a `count`×`count` grid, each cell
/// seeded with an offset (`jitter`, in cells), a rotation (`rotation_jitter`, in
/// turns) and a scale (`scale_jitter`); overlaps keep the brighter stamp. Always
/// tiles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub count: f32,
    #[serde(default)]
    pub jitter: f32,
    #[serde(default)]
    pub rotation_jitter: f32,
    #[serde(default)]
    pub scale_jitter: f32,
    #[serde(default)]
    pub seed: u32,
}

/// Domain warp: sample input A displaced by input B's R/G, centred on 0.5 and scaled
/// by `strength` in **tile units**; wrapped, so it tiles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    pub strength: f32,
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

/// A separable box blur (wraps at the edges). `radius` is a **fraction of the tile
/// width** (`0.01` = 1%), so it looks the same at any resolution (#394).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blur {
    pub radius: f32,
}

/// Height → cavity mask: `1 − max(blur(h) − h, 0)` with a `radius` blur (tile
/// fraction, as `blur`). White on flat and raised ground, dark in crevices (#408).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cavity {
    pub radius: f32,
}

/// Height → curvature: `0.5 + strength × (h − mean of h on a ring of `radius`)` (tile
/// units). Above 0.5 convex (edge wear), below concave (grime) (#408).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curvature {
    pub radius: f32,
    #[serde(default = "one_f32")]
    pub strength: f32,
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

fn quarter() -> f32 {
    0.25
}

fn default_mortar() -> f32 {
    0.05
}
