//! src/procgen/recipe/kinds.rs — the enum-valued params of [`super::OpKind`]: blend
//! modes, math functions, and each generator's `kind` / `output` choices. Every one
//! serializes as its snake_case tag, the string a recipe writes.

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

/// What the brick op outputs per pixel. Mortar reads 0 in every output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrickOutput {
    /// Brick faces 1, mortar 0.
    #[default]
    Mask,
    /// A seeded value in `[0, 1)` per brick: per-brick colour / roughness variation.
    Random,
    /// Distance to the brick's nearest edge, 1 on its centre line: a rounded-brick
    /// height for `bump_to_normal`.
    Bevel,
}

/// Which primitive [`super::OpKind::Shape`] draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    /// An ellipse filling `size` (a circle when both sides match).
    Circle,
    /// A sharp-cornered rectangle.
    Rect,
    /// A rectangle whose corners are rounded by `roundness`.
    RoundedRect,
    /// A horizontal stroke with round caps: `size` is its length and thickness.
    Line,
}
