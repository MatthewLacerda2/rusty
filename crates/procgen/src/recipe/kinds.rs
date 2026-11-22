use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Mix,
    Add,
    Multiply,
    Screen,
    Overlay,
    Subtract,
    Difference,
    Lighten,
    Darken,
    /// a / b; a channel of b at 0 leaves a.
    Divide,
}

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
    /// Argument in radians.
    Sin,
    Cos,
    /// Radians in `-π..π` of the point `(value, c)`: Blender's *Arctan2*.
    Atan2,
}

impl MathOp {
    pub fn is_unary(self) -> bool {
        matches!(
            self,
            MathOp::Abs | MathOp::Fract | MathOp::Sqrt | MathOp::Sin | MathOp::Cos
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseKind {
    Perlin,
    Fbm,
    /// `1 − |n|`, squared, summed over octaves: sharp crests (rock veins, scratches).
    Ridged,
    Turbulence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveKind {
    Bands,
    Rings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientKind {
    /// Does not tile (hard edge at the wrap).
    Linear,
    LinearTiling,
    Radial,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoronoiOutput {
    #[default]
    Distance,
    Cells,
    F2,
    Edges,
    SmoothF1,
}

/// What the brick op outputs per pixel. Mortar reads 0 in every output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrickOutput {
    #[default]
    Mask,
    /// A seeded value in `[0, 1)` per brick: per-brick colour / roughness variation.
    Random,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Circle,
    Rect,
    RoundedRect,
    Line,
    Polygon,
    Star,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEncoding {
    #[default]
    Srgb,
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoronoiMetric {
    #[default]
    Euclidean,
    Manhattan,
    Chebyshev,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RampInterpolation {
    #[default]
    Linear,
    Constant,
    Ease,
    Cardinal,
    BSpline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlurKind {
    Box,
    #[default]
    Gaussian,
}
