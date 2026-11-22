use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use super::reflection_probe::clamp_resolution;

mod bake;

pub use bake::{page_edge, BakeSettings, Denoiser, MAX_PAGE, MIN_PAGE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSettings {
    pub lightmaps: BakeSettings,
    pub reflections: ReflectionSettings,
    #[serde(skip_serializing_if = "DiffuseModel::is_lambert")]
    pub diffuse_model: DiffuseModel,
    /// Takes effect at the next bake.
    #[serde(skip_serializing_if = "MixedLightingMode::is_baked_indirect")]
    pub mixed_lighting_mode: MixedLightingMode,
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum MixedLightingMode {
        #[default]
        BakedIndirect = "BakedIndirect",
        Shadowmask = "Shadowmask",
    }
}

impl MixedLightingMode {
    pub fn is_baked_indirect(&self) -> bool {
        *self == Self::BakedIndirect
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffuseModel {
    #[default]
    Lambert,
    HalfLambert,
}

impl DiffuseModel {
    pub const ALL: [Self; 2] = [Self::Lambert, Self::HalfLambert];

    pub fn name(self) -> &'static str {
        match self {
            Self::Lambert => "Lambert",
            Self::HalfLambert => "HalfLambert",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    pub fn is_lambert(&self) -> bool {
        *self == Self::Lambert
    }

    /// `forward/lighting.wgsl`'s `diffuse_response` mirrors this: change the two together.
    pub fn response(self, cos: f32) -> f32 {
        match self {
            Self::Lambert => cos.max(0.0),
            Self::HalfLambert => {
                let w = (cos * 0.5 + 0.5).clamp(0.0, 1.0);
                w * w
            }
        }
    }

    /// The squared wrap is quadratic in N·L, so L2 holds it exactly: its zonal kernel
    /// (4π/3, 2π/3, 2π/15) over Lambert's (π, 2π/3, π/4).
    pub fn sh_band_scale(self) -> [f32; 3] {
        match self {
            Self::Lambert => [1.0; 3],
            Self::HalfLambert => [4.0 / 3.0, 1.0, 8.0 / 15.0],
        }
    }
}

impl LightingSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectionSettings {
    pub resolution: u32,
    pub budget_mb: u32,
    pub max_probes: u32,
}

impl Default for ReflectionSettings {
    fn default() -> Self {
        Self {
            // Unreal's and Unity's default: 128 KiB a cube in BC6H with mips, 1 MiB in half
            // floats, so the budget keeps the 32 the renderer shades resident either way.
            resolution: 128,
            budget_mb: 256,
            max_probes: 512,
        }
    }
}

impl ReflectionSettings {
    pub const BUDGET_MB: RangeInclusive<u32> = 8..=2048;
    pub const MAX_PROBES: RangeInclusive<u32> = 1..=4096;

    pub fn clamp(&mut self) {
        let (lo, hi) = (*Self::MAX_PROBES.start(), *Self::MAX_PROBES.end());
        self.max_probes = self.max_probes.clamp(lo, hi);
        self.resolution = clamp_resolution(self.resolution);
        self.budget_mb = self
            .budget_mb
            .clamp(*Self::BUDGET_MB.start(), *Self::BUDGET_MB.end());
    }

    pub fn budget_bytes(&self) -> u64 {
        u64::from(self.budget_mb) << 20
    }
}

#[cfg(test)]
mod diffuse_tests;
#[cfg(test)]
mod direct_samples_tests;
