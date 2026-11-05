//! The scene's lighting settings (#832): what Generate Lighting bakes with, stored
//! with the scene the way Unity keeps a Lighting Settings asset per scene, so a
//! rebake never asks for the knobs again.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use super::lightmap::BakeSettings;
use super::reflection_probe::clamp_resolution;

/// Unity's Lighting Settings, the part rusty has: the lightmap bake's knobs, the
/// reflection probes' defaults, the diffuse model every lit path shades with and
/// what a `Mixed` light means (its Mixed Lighting Mode, #1695).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSettings {
    /// Texels per unit, samples, bounces, seed, filter and the directional mode.
    pub lightmaps: BakeSettings,
    /// The reflection probes' default resolution and the memory they may hold.
    pub reflections: ReflectionSettings,
    /// How a lamp's direct light falls off across a surface (#1698): realtime
    /// shading, the lightmap bake and the light-probe bake all read it, so one lamp
    /// lights the side facing it the same Baked or Realtime. Left out of the file at
    /// Lambert, so a scene saved before it existed round-trips unchanged.
    #[serde(skip_serializing_if = "DiffuseModel::is_lambert")]
    pub diffuse_model: DiffuseModel,
    /// What the bake and the frame make of a `Mixed` light (#1695), Unity's
    /// *Lighting Mode*: `BakedIndirect` (the default, left out of the file) or
    /// `Shadowmask`. A change takes effect at the next bake: the frame follows
    /// what the last bake recorded (`LightmapSet::channels`).
    #[serde(skip_serializing_if = "MixedLightingMode::is_baked_indirect")]
    pub mixed_lighting_mode: MixedLightingMode,
}

crate::core::named_enum! {
    /// Unity's scene-level *Mixed Lighting Mode* (#1695): what a `Mixed` light
    /// bakes and how it shades.
    ///
    /// * `BakedIndirect` — its bounce is baked, its direct light and shadows are
    ///   realtime: no shadow past the shadow distance. The default.
    /// * `Shadowmask` — also bakes each `Mixed` light's static occlusion, per
    ///   texel into a shadowmask page beside each lightmap page and per light
    ///   probe for dynamic objects, so its direct light (diffuse and specular) is
    ///   realtime but its static shadows hold at any distance. At most four
    ///   overlapping `Mixed` lights share a texel (one channel each); a light that
    ///   gets no channel bakes as `Baked`. Which casters draw its realtime shadow is
    ///   the quality preset's `shadowmask_mode` (Unity's `QualitySettings.shadowmaskMode`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum MixedLightingMode {
        #[default]
        BakedIndirect = "BakedIndirect",
        Shadowmask = "Shadowmask",
    }
}

impl MixedLightingMode {
    /// True for the default, [`MixedLightingMode::BakedIndirect`]: the scene file
    /// then leaves it out, so a scene saved before it existed round-trips unchanged.
    pub fn is_baked_indirect(&self) -> bool {
        *self == Self::BakedIndirect
    }
}

/// The diffuse response to a lamp's direct light, scene-wide (#1698). It applies
/// wherever a lamp lights a surface: realtime shading, lightmap texels, the surfaces
/// a bake's bounce rays hit and those a probe capture renders, so global
/// illumination gets brighter under [`DiffuseModel::HalfLambert`] too. Only the
/// transfer between surfaces (a bounce lighting a texel) and the sky stay Lambert.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffuseModel {
    /// The physical `max(N·L, 0)`, what every PBR engine shades with.
    #[default]
    Lambert,
    /// The retro squared wrap `((N·L + 1) / 2)²` (GoldSrc's look): a quarter of the
    /// light at the terminator, none only straight behind, and about a third more
    /// diffuse energy than Lambert.
    HalfLambert,
}

impl DiffuseModel {
    /// Every model, in the editor's order.
    pub const ALL: [Self; 2] = [Self::Lambert, Self::HalfLambert];

    /// The name `Lighting.GetSettings` returns and `SetSettings` takes.
    pub fn name(self) -> &'static str {
        match self {
            Self::Lambert => "Lambert",
            Self::HalfLambert => "HalfLambert",
        }
    }

    /// The model called `name`, in any case, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    /// True for the default, [`DiffuseModel::Lambert`]: the scene file leaves it out.
    pub fn is_lambert(&self) -> bool {
        *self == Self::Lambert
    }

    /// The share of a lamp's radiance a surface receives at `cos` = N·L, the
    /// factor `calculate_pbr` (`forward/lighting.wgsl`) multiplies its diffuse by.
    pub fn response(self, cos: f32) -> f32 {
        match self {
            Self::Lambert => cos.max(0.0),
            Self::HalfLambert => {
                let w = (cos * 0.5 + 0.5).clamp(0.0, 1.0);
                w * w
            }
        }
    }

    /// Per SH band, what a lamp's delta must be scaled by so the probes' cosine-lobe
    /// read-back (`Sh9::eval`) gives this model's response. The squared wrap is a
    /// quadratic in N·L, so L2 holds it exactly: its zonal kernel is
    /// (4π/3, 2π/3, 2π/15) against Lambert's (π, 2π/3, π/4).
    pub fn sh_band_scale(self) -> [f32; 3] {
        match self {
            Self::Lambert => [1.0; 3],
            Self::HalfLambert => [4.0 / 3.0, 1.0, 8.0 / 15.0],
        }
    }
}

impl LightingSettings {
    /// True when every knob is at its default: the scene file then leaves the block
    /// out, so a scene that never touched them saves byte for byte as before.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The reflection probes' scene-wide settings (#1584).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectionSettings {
    /// The face size a probe without its own `resolution` bakes at, a power of two.
    pub resolution: u32,
    /// The megabytes of probe cubes the renderer keeps resident at once: the nearest
    /// boxes to the camera fill it, and an object whose probe is past it reflects the
    /// next probe, or the sky.
    pub budget_mb: u32,
    /// The most probes `Lighting.Generate()` places: over it, the tiles a large
    /// room is cut into widen until the level fits.
    pub max_probes: u32,
}

impl Default for ReflectionSettings {
    /// 512-texel cubes, one good size for every probe; a 256 MB budget holds 16 of
    /// them (16 MB each in half floats with their mips, #1696): 14 shaded and 2 spare,
    /// the rooms around a player (32, the cap, at 256 texels). 512
    /// probes cover a Dust2-sized level (200 m of 20 m rooms takes 400 at 12 m tiles).
    fn default() -> Self {
        Self {
            resolution: 512,
            budget_mb: 256,
            max_probes: 512,
        }
    }
}

impl ReflectionSettings {
    /// The budgets a write accepts, in megabytes.
    pub const BUDGET_MB: RangeInclusive<u32> = 8..=2048;
    /// The probe ceilings a write accepts.
    pub const MAX_PROBES: RangeInclusive<u32> = 1..=4096;

    /// Hold every knob to its range: a power-of-two resolution, a budget in
    /// [`Self::BUDGET_MB`] and a ceiling in [`Self::MAX_PROBES`].
    pub fn clamp(&mut self) {
        let (lo, hi) = (*Self::MAX_PROBES.start(), *Self::MAX_PROBES.end());
        self.max_probes = self.max_probes.clamp(lo, hi);
        self.resolution = clamp_resolution(self.resolution);
        self.budget_mb = self
            .budget_mb
            .clamp(*Self::BUDGET_MB.start(), *Self::BUDGET_MB.end());
    }

    /// The budget in bytes.
    pub fn budget_bytes(&self) -> u64 {
        u64::from(self.budget_mb) << 20
    }
}

#[cfg(test)]
#[path = "diffuse_tests.rs"]
mod diffuse_tests;
