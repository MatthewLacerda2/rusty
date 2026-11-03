//! The scene's lighting settings (#832): what Generate Lighting bakes with, stored
//! with the scene the way Unity keeps a Lighting Settings asset per scene, so a
//! rebake never asks for the knobs again.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use super::lightmap::BakeSettings;
use super::reflection_probe::clamp_resolution;

/// Unity's Lighting Settings, the part rusty has: the lightmap bake's knobs, the
/// reflection probes' defaults and the diffuse model every lit path shades with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSettings {
    /// Texels per unit, samples, bounces, seed, filter and the directional mode.
    pub lightmaps: BakeSettings,
    /// The reflection probes' default resolution and the memory they may hold.
    pub reflections: ReflectionSettings,
    /// How a lamp's direct light falls off across a surface (#1698): realtime
    /// shading, the lightmap bake and the light-probe bake all read it, so one lamp
    /// looks the same Baked or Realtime.
    pub diffuse_model: DiffuseModel,
}

/// The diffuse response to a lamp's direct light, scene-wide (#1698). Indirect light
/// (bounce, sky, probes' captured surfaces) stays Lambert under either.
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

    /// The model called `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
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
    /// 512-texel cubes, one good size for every probe; a 128 MB budget holds 16 of
    /// them (8 MB each with their mips): 14 shaded and 2 spare, enough for the rooms
    /// around a player. 512
    /// probes cover a Dust2-sized level (200 m of 20 m rooms takes 400 at 12 m tiles).
    fn default() -> Self {
        Self {
            resolution: 512,
            budget_mb: 128,
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
