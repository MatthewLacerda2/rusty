//! src/scene/settings/ambient.rs — the ambient light a frame draws with: Unity's
//! `RenderSettings.ambientLight`, unless an active Ambient light entity overrides it.
//!
//! One rule, two readers: the renderer's lighting uniform
//! (`render::draw::lighting`) and the Scene Settings panel, which starts a sky
//! colour from the one drawn now (`SkySettings::resolved`, #1620).

use glam::Vec3;

use crate::scene::{LightType, Scene};

impl Scene {
    /// The ambient colour and intensity drawn: the last active Ambient light
    /// entity's, else the scene's own `ambient_color` and `ambient_intensity`.
    pub fn drawn_ambient(&self) -> (Vec3, f32) {
        let mut ambient = (self.ambient_color, self.ambient_intensity);
        for id in self.world.ids_with_light() {
            let light = self.world.light(id).expect("id came from ids_with_light");
            if light.light_type == LightType::Ambient && self.world.is_active_in_hierarchy(id) {
                ambient = (light.color, light.intensity);
            }
        }
        ambient
    }
}
