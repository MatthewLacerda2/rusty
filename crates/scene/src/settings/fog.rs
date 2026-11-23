//! The scene's distance and height fog, applied by `common.wgsl`'s `fog_factor` (its modes in
//! `fog/height.wgsl`) to every world-space pass. Every write goes through
//! `edit::authoring::fog`, where its clamps live.

use glam::Vec3;
use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum FogMode {
        #[default]
        Off = "Off",
        Linear = "Linear",
        /// `1 - e^(-density·d)`: thick fog that never quite reaches full.
        Exponential = "Exponential",
        /// `1 - e^(-(density·d)²)`: clear near the camera, then closes in fast.
        ExponentialSquared = "ExponentialSquared",
    }
}

impl FogMode {
    /// `fog_factor`'s `mode`, `0` off: change the two together.
    pub fn to_index(self) -> u32 {
        match self {
            FogMode::Off => 0,
            FogMode::Linear => 1,
            FogMode::Exponential => 2,
            FogMode::ExponentialSquared => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FogSettings {
    pub mode: FogMode,
    pub color: Vec3,
    pub density: f32,
    /// World units from the camera where fog begins, in every mode.
    pub start: f32,
    pub end: f32,
    /// Per world unit above `base_height`; `0` is uniform fog.
    pub height_falloff: f32,
    pub base_height: f32,
}

impl Default for FogSettings {
    fn default() -> Self {
        Self {
            mode: FogMode::Off,
            color: Vec3::new(0.6, 0.65, 0.7),
            density: 0.02,
            start: 10.0,
            end: 100.0,
            height_falloff: 0.0,
            base_height: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_without_fog_loads_it_off() {
        let fog: FogSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(fog, FogSettings::default());
        assert_eq!(fog.mode, FogMode::Off);
    }

    #[test]
    fn indices_are_stable() {
        for (i, m) in FogMode::ALL.into_iter().enumerate() {
            assert_eq!(m.to_index(), i as u32);
        }
    }
}
