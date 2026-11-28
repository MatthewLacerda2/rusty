//! The settings that are a scene's own while it is active: the World holds the active one's.

use glam::Vec3;

use crate::Scene;

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSettings {
    pub skybox_path: String,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub ambient: crate::AmbientSettings,
    pub nav_settings: crate::nav_settings::NavMeshSettings,
    pub fog: crate::FogSettings,
    pub sky: crate::SkySettings,
    pub lighting_settings: crate::lighting::LightingSettings,
}

impl Scene {
    pub fn settings(&self) -> SceneSettings {
        SceneSettings {
            skybox_path: self.skybox_path.clone(),
            ambient_color: self.ambient_color,
            ambient_intensity: self.ambient_intensity,
            ambient: self.ambient,
            nav_settings: self.nav_settings.clone(),
            fog: self.fog,
            sky: self.sky,
            lighting_settings: self.lighting_settings,
        }
    }

    pub(super) fn put_settings(&mut self, settings: SceneSettings) {
        self.skybox_path = settings.skybox_path;
        self.ambient_color = settings.ambient_color;
        self.ambient_intensity = settings.ambient_intensity;
        self.ambient = settings.ambient;
        self.nav_settings = settings.nav_settings;
        self.fog = settings.fog;
        self.sky = settings.sky;
        self.lighting_settings = settings.lighting_settings;
    }
}
