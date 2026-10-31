//! src/scene/settings/sky.rs — the procedural sky's settings (#1620).
//!
//! Unity's *Skybox/Procedural* material (sky tint, ground) and HDRP's *Cloud Layer*
//! (coverage, opacity), one per scene, saved with it. They shape the sky the renderer
//! draws when no panorama is set (`common::procedural_sky`), and so what a surface
//! reflects of it (the sky reflection cube, re-baked when they change, #1553) and
//! what a reflection-probe bake captures.
//!
//! Each colour is optional: unset, it follows the scene's ambient colour as the sky
//! always has (the zenith *is* the ambient colour, the horizon that lifted toward
//! white, the ground a quarter of the zenith), so the defaults draw today's sky
//! exactly and a scripted ambient (a day cycle) still tints it every frame.
//!
//! Pure data. Every write goes through `scene::authoring::environment`, shared by the
//! Scene Settings panel and `Lighting.Set*`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// The default cloud coverage: the cloud layer the procedural sky has always drawn.
pub const DEFAULT_CLOUD_COVERAGE: f32 = 0.5;
/// The default cloud opacity: likewise.
pub const DEFAULT_CLOUD_OPACITY: f32 = 0.6;

/// The procedural sky's look. `#[serde(default)]` loads a partial block (or none)
/// with the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkySettings {
    /// Linear colour straight up; `None` follows the ambient colour.
    pub zenith: Option<Vec3>,
    /// Linear colour at the horizon; `None` is the zenith lifted 60% toward white.
    pub horizon: Option<Vec3>,
    /// Linear colour below the horizon; `None` is a quarter of the zenith.
    pub ground: Option<Vec3>,
    /// How much of the sky the clouds cover, `0..=1`: `0` is a clear sky, `1`
    /// overcast.
    pub cloud_coverage: f32,
    /// How opaque a cloud is, `0..=1`: `0` hides the layer.
    pub cloud_opacity: f32,
}

impl Default for SkySettings {
    fn default() -> Self {
        Self {
            zenith: None,
            horizon: None,
            ground: None,
            cloud_coverage: DEFAULT_CLOUD_COVERAGE,
            cloud_opacity: DEFAULT_CLOUD_OPACITY,
        }
    }
}

impl SkySettings {
    /// Every knob at its default: the scene file leaves the block out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The zenith, horizon and ground colours drawn under ambient colour `ambient`:
    /// each set one, else derived as `common::procedural_sky` derives it. What the
    /// Scene Settings panel starts a colour from when it is first set.
    pub fn resolved(&self, ambient: Vec3) -> [Vec3; 3] {
        let zenith = self.zenith.unwrap_or(ambient);
        let horizon = self.horizon.unwrap_or(zenith.lerp(Vec3::ONE, 0.6));
        let ground = self.ground.unwrap_or(zenith * 0.25);
        [zenith, horizon, ground]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_without_a_sky_block_loads_the_default() {
        let sky: SkySettings = serde_json::from_str("{}").unwrap();
        assert!(sky.is_default());
        let partial: SkySettings = serde_json::from_str(r#"{"cloud_coverage":0}"#).unwrap();
        assert_eq!(partial.cloud_opacity, DEFAULT_CLOUD_OPACITY);
        assert_eq!(partial.zenith, None);
    }

    #[test]
    fn the_sky_saves_and_loads_with_the_scene() {
        use crate::scene::{apply_scene_data, to_scene_data, Scene, SceneData};
        let mut scene = Scene::new();
        scene.sky.cloud_coverage = 0.9;
        scene.sky.ground = Some(Vec3::new(0.2, 0.15, 0.1));
        let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
        let mut back = Scene::new();
        apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
        assert_eq!(back.sky, scene.sky);
    }

    #[test]
    fn unset_colours_derive_from_the_ambient_then_the_zenith() {
        let ambient = Vec3::new(0.4, 0.5, 0.6);
        let [z, h, g] = SkySettings::default().resolved(ambient);
        assert_eq!((z, g), (ambient, ambient * 0.25));
        assert!(h.abs_diff_eq(Vec3::new(0.76, 0.8, 0.84), 1e-6));
        let set = SkySettings {
            zenith: Some(Vec3::ONE),
            ..SkySettings::default()
        };
        assert_eq!(set.resolved(ambient)[2], Vec3::splat(0.25));
    }

    #[test]
    fn an_untouched_sky_stays_out_of_the_file() {
        let scene = crate::scene::Scene::new();
        let json = serde_json::to_string(&crate::scene::to_scene_data(&scene)).unwrap();
        assert!(!json.contains("\"sky\""), "{json}");
    }
}
