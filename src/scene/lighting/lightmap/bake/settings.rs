//! The bake's knobs ([`BakeSettings`]), Unity's Lightmapping settings in
//! miniature, and the one place their ranges live.

use std::ops::RangeInclusive;

use super::super::atlas::{page_edge, MAX_PAGE, MIN_PAGE};
use super::Denoiser;

/// The bake's knobs, Unity's Lightmapping settings in miniature. The scene stores
/// them in its lighting settings (#832); a field a scene file omits takes its default.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BakeSettings {
    /// Lightmap texels per world unit (Unity's "Lightmap Resolution").
    pub texels_per_unit: f32,
    /// Paths traced per texel; more is smoother and slower.
    pub samples: u32,
    /// Surfaces a path bounces off before it stops.
    pub bounces: u32,
    /// The seed every texel's sample stream derives from.
    pub seed: u64,
    /// Unity's *Max Lightmap Size* (#1704): the atlas page's edge, in texels, a
    /// power of two in [`Self::MAX_LIGHTMAP_SIZE`]. A mesh's lightmap follows its
    /// area × `texels_per_unit` up to the largest map such a page holds
    /// ([`max_map_edge`](super::super::atlas::max_map_edge)); the packer picks the smallest page that holds the bake,
    /// up to this edge, then adds pages. Read through [`Self::page`].
    pub max_lightmap_size: u32,
    /// What cleans the baked bounce (#1884, Unity's *Denoiser* and *Filter*): OIDN
    /// then the Gaussian below, the Gaussian alone, or nothing. Baked direct light
    /// is never cleaned.
    pub denoiser: Denoiser,
    /// Radius, in texels, of the Gaussian that smooths the baked bounce (Unity's
    /// indirect filter): alone under [`Denoiser::Filter`] or where OIDN cannot run,
    /// after OIDN under `Oidn`; 0 leaves it as traced or as OIDN gave it.
    pub filter_radius: u32,
    /// Also bake each texel's dominant incoming light direction and its `Baked`
    /// lamps' share (#810, Unity's *Directional* mode; #1695), so normal maps
    /// reshape baked light and `Baked` lamps give a lightmapped surface one
    /// specular highlight. Off saves the direction pages' memory; the colour
    /// lightmaps are the same either way.
    pub directional: bool,
    /// Unity's *Backface Tolerance* (#1949): the share of a texel's validity probes
    /// that must reach a front face (or escape) for its light to count. A texel
    /// below it sits inside geometry, and its own chart's valid texels fill it.
    pub backface_tolerance: f32,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            texels_per_unit: 8.0,
            samples: Self::DEFAULT_SAMPLES,
            bounces: 3,
            seed: 0,
            max_lightmap_size: 1024,
            denoiser: Denoiser::Oidn,
            filter_radius: 3,
            directional: true,
            backface_tolerance: Self::DEFAULT_BACKFACE_TOLERANCE,
        }
    }
}

impl BakeSettings {
    /// The default paths per texel: the fewest at which an OIDN bake (OIDN, then
    /// the Gaussian) is as close to a converged one as the Gaussian alone was at
    /// its old default of 128, within a point of mean relative error, in about 60%
    /// of the time (#1884, `dev::bake::denoise::quality_tests`; 32 missed by 1.3 to
    /// 1.7 points). A bake that falls back to the Gaussian alone is blurrier and
    /// blotchier at this count.
    pub const DEFAULT_SAMPLES: u32 = 64;
    /// Unity's default *Backface Tolerance*: a texel is valid when nine in ten of
    /// its probes see front faces (#1949).
    pub const DEFAULT_BACKFACE_TOLERANCE: f32 = 0.9;
    /// The backface tolerances a bake takes, as Unity's slider.
    pub const BACKFACE_TOLERANCE: RangeInclusive<f32> = 0.0..=1.0;
    /// The texel densities a bake takes (Unity's Lightmap Resolution, in miniature).
    pub const TEXELS_PER_UNIT: RangeInclusive<f32> = 0.25..=64.0;
    /// The samples per texel a bake takes.
    pub const SAMPLES: RangeInclusive<u32> = 1..=4096;
    /// The bounces a bake takes.
    pub const BOUNCES: RangeInclusive<u32> = 1..=16;
    /// The page edges a bake takes (Unity's 32–4096), each a power of two.
    pub const MAX_LIGHTMAP_SIZE: RangeInclusive<u32> = MIN_PAGE..=MAX_PAGE;

    /// The atlas page edge this bake packs into: [`Self::max_lightmap_size`] held
    /// to a power of two in [`Self::MAX_LIGHTMAP_SIZE`] (a hand-edited file may
    /// hold anything).
    pub fn page(&self) -> u32 {
        page_edge(self.max_lightmap_size)
    }

    /// These settings with each authored knob clamped into its range: the one place
    /// those limits live, applied by `Lighting.SetSettings` / `BakeLightmaps` and
    /// spanned by the editor's fields (#993). A non-finite density or backface
    /// tolerance takes the default.
    pub fn sanitized(self) -> Self {
        let texels = match self.texels_per_unit.is_finite() {
            true => self.texels_per_unit,
            false => Self::default().texels_per_unit,
        };
        let clamp = |v: u32, r: RangeInclusive<u32>| v.clamp(*r.start(), *r.end());
        let tolerance = match self.backface_tolerance.is_finite() {
            true => self.backface_tolerance,
            false => Self::DEFAULT_BACKFACE_TOLERANCE,
        };
        let (low, high) = Self::BACKFACE_TOLERANCE.into_inner();
        Self {
            texels_per_unit: texels
                .clamp(*Self::TEXELS_PER_UNIT.start(), *Self::TEXELS_PER_UNIT.end()),
            samples: clamp(self.samples, Self::SAMPLES),
            bounces: clamp(self.bounces, Self::BOUNCES),
            max_lightmap_size: self.page(),
            backface_tolerance: tolerance.clamp(low, high),
            ..self
        }
    }
}
