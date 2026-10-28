//! src/scene/settings/fog.rs — the scene's distance + height fog setting (#437).
//!
//! Unity's Lighting-window fog: one per scene, saved with it, applied by the
//! renderer to every world-space pass (opaque, transparent, particles, decals, and
//! the sky's horizon) through one shared WGSL function (`common.wgsl::apply_fog`).
//! It sits beside the skybox and ambient scalars rather than on the
//! post-processing volume: fog is the scene's atmosphere, it must hold with no
//! volume in the scene, and the sky it blends into is scene-level too.
//!
//! Pure data. Every write goes through `scene::authoring::fog`, the one place its
//! clamps live, shared by the Scene Settings panel and `Graphics.SetFog*`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// How fog thickens with distance from the camera.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum FogMode {
        /// No fog — the default.
        #[default]
        Off = "Off",
        /// Ramps from none at `start` to full at `end`.
        Linear = "Linear",
        /// `1 - e^(-density·d)`: thick fog that never quite reaches full.
        Exponential = "Exponential",
        /// `1 - e^(-(density·d)²)`: clear near the camera, then closes in fast.
        ExponentialSquared = "ExponentialSquared",
    }
}

impl FogMode {
    /// Stable index handed to the shader as a `u32` (`0` means off).
    pub fn to_index(self) -> u32 {
        match self {
            FogMode::Off => 0,
            FogMode::Linear => 1,
            FogMode::Exponential => 2,
            FogMode::ExponentialSquared => 3,
        }
    }
}

/// The scene's fog. `#[serde(default)]` loads a partial block (or none) with the
/// defaults below.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FogSettings {
    pub mode: FogMode,
    /// Linear-space colour distant surfaces fade into.
    pub color: Vec3,
    /// Thickness for the exponential modes (per world unit).
    pub density: f32,
    /// Distance from the camera where fog begins, in every mode.
    pub start: f32,
    /// Distance where linear fog is full.
    pub end: f32,
    /// How fast fog thins above `base_height`, per world unit. `0` is uniform fog;
    /// larger values pool it low, like smoke on a warehouse floor.
    pub height_falloff: f32,
    /// World height below which fog is at full thickness.
    pub base_height: f32,
}

impl Default for FogSettings {
    /// Off, with a neutral haze ready for when a scene turns it on: a grey-blue a
    /// touch lighter than the default ambient, 0.02 density (half-fogged at ~35 m),
    /// linear over 10–100 m, uniform in height.
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
    fn fog_saves_and_loads_with_the_scene() {
        use crate::scene::{apply_scene_data, to_scene_data, Scene, SceneData};
        let mut scene = Scene::new();
        scene.fog.mode = FogMode::Linear;
        scene.fog.end = 42.0;
        let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
        let mut back = Scene::new();
        apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
        assert_eq!(back.fog, scene.fog);
    }

    #[test]
    fn indices_are_stable() {
        for (i, m) in FogMode::ALL.into_iter().enumerate() {
            assert_eq!(m.to_index(), i as u32);
        }
    }
}
