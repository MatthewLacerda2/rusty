//! src/procgen/recipe/params/generators.rs — the generator ops' params (no inputs).

use serde::{Deserialize, Serialize};

use super::super::color;
use super::super::{
    BrickOutput, GradientKind, ImageEncoding, NoiseKind, ShapeKind, VoronoiMetric, VoronoiOutput,
    WaveKind,
};
use super::{default_mortar, half, one_f32, one_u32, quarter, two};

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
/// regular grid, 1 = anywhere in its cell); `metric` is how distance is measured.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Voronoi {
    pub scale: f32,
    #[serde(default)]
    pub output: VoronoiOutput,
    #[serde(default)]
    pub metric: VoronoiMetric,
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

/// An image file as a source (#1491), Blender's *Image Texture* node. `path` is
/// project-relative; the caller reads and decodes the file
/// ([`crate::procgen::Sources`]), since procgen reads no files. It is resampled
/// bilinearly, wrapped, to the recipe's resolution; its tiling is the file's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageTexture {
    pub path: String,
    #[serde(default)]
    pub encoding: ImageEncoding,
}
