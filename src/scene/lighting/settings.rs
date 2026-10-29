//! The scene's lighting settings (#832): what Generate Lighting bakes with, stored
//! with the scene the way Unity keeps a Lighting Settings asset per scene, so a
//! rebake never asks for the knobs again.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use super::lightmap::BakeSettings;
use super::reflection_probe::clamp_resolution;

/// Unity's Lighting Settings, the part rusty has: the lightmap bake's knobs and the
/// reflection probes' defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSettings {
    /// Texels per unit, samples, bounces, seed, filter and the directional mode.
    pub lightmaps: BakeSettings,
    /// The reflection probes' default resolution and the memory they may hold.
    pub reflections: ReflectionSettings,
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
}

impl Default for ReflectionSettings {
    /// 512-texel cubes, one good size for every probe; a 128 MB budget holds 15 of
    /// them (about 8.4 MB each with their mips), enough for the rooms around a player.
    fn default() -> Self {
        Self {
            resolution: 512,
            budget_mb: 128,
        }
    }
}

impl ReflectionSettings {
    /// The budgets a write accepts, in megabytes.
    pub const BUDGET_MB: RangeInclusive<u32> = 8..=2048;

    /// Hold both knobs to their ranges: a power-of-two resolution and a budget in
    /// [`Self::BUDGET_MB`].
    pub fn clamp(&mut self) {
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
