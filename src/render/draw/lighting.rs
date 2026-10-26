//! Construction of the per-frame `LightingUniform` from a scene's lights and SSR
//! (Post Processing) settings. Free builders split out of `draw.rs` to keep each
//! file under the size cap; `Renderer::build_lighting_uniform` orchestrates them
//! (behavior unchanged).

use glam::Vec3;

use crate::render::gpu::uniforms::MAX_DIRECTIONAL_LIGHTS;
use crate::render::ibl::reflection_probes::MAX_REFLECTION_PROBES;
use crate::render::{AmbientLightUniform, DirectionalLightUniform, LightingUniform};
use crate::scene::{LightType, Scene};

/// The base lighting uniform before scene lights/SSR are scanned in.
pub(crate) fn default_lighting_uniform(scene: &Scene) -> LightingUniform {
    LightingUniform {
        ambient: AmbientLightUniform {
            color: scene.ambient_color.to_array(),
            intensity: scene.ambient_intensity,
        },
        dir_lights: [bytemuck::Zeroable::zeroed(); MAX_DIRECTIONAL_LIGHTS],
        num_dir_lights: 0,
        ssr_active: 0.0,
        ssr_quality: 0.0,
        ssr_temporal_upsampling: 0.0,
        sky_textured: 0.0,
        _sky_pad: [0.0; 3],
        refl_probes: [Default::default(); MAX_REFLECTION_PROBES],
    }
}

/// Fill the ambient and directional slots from the active light entities. The last
/// ambient wins. Directional lights fill [`MAX_DIRECTIONAL_LIGHTS`] slots in entity
/// order, except that the last one (the shadow pass's sun) always takes slot 0.
/// Point and spot lights go to the cluster lights instead (#434). Only the
/// directional lights [`LightMode::renders_live`] keeps for a frame or a bake
/// `capture` are uploaded (#809); the ambient ignores modes.
///
/// [`LightMode::renders_live`]: crate::components::LightMode::renders_live
pub(crate) fn apply_scene_lights(
    lighting_uniform: &mut LightingUniform,
    scene: &Scene,
    capture: bool,
) {
    let mut dirs = Vec::new();
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let light = scene.world.light(id).expect("id came from ids_with_light");
        match light.light_type {
            LightType::Ambient => {
                lighting_uniform.ambient = AmbientLightUniform {
                    color: light.color.to_array(),
                    intensity: light.intensity,
                };
            }
            LightType::Directional if light.mode.renders_live(capture) => {
                dirs.push(DirectionalLightUniform {
                    direction: scene.world.transform_direction(id, Vec3::NEG_Z).to_array(),
                    _pad1: 0.0,
                    color: light.color.to_array(),
                    intensity: light.intensity,
                    // `Baked` (#438), only ever uploaded in a bake capture: a
                    // lightmapped surface skips it, its lightmap holds it.
                    baked: f32::from(u8::from(light.mode.bakes_direct())),
                    _pad2: [0.0; 3],
                })
            }
            LightType::Directional | LightType::Point | LightType::Spotlight => {}
        }
    }
    if let Some(sun) = dirs.pop() {
        dirs.insert(0, sun);
    }
    dirs.truncate(MAX_DIRECTIONAL_LIGHTS);
    lighting_uniform.dir_lights[..dirs.len()].copy_from_slice(&dirs);
    lighting_uniform.num_dir_lights = dirs.len() as u32;
}

/// Fold active Post Processing (SSR) components into the uniform; last one wins.
pub(crate) fn apply_ssr_settings(lighting_uniform: &mut LightingUniform, scene: &Scene) {
    let mut ssr_active = 0.0;
    let mut ssr_quality = 2.0; // High default
    let mut ssr_temporal = 0.0;
    for id in scene.world.ids_with_post_processing() {
        if !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let vc = scene
            .world
            .post_processing(id)
            .expect("id came from ids_with_post_processing");
        if vc.ssr_active {
            ssr_active = 1.0;
        }
        ssr_quality = match vc.ssr_quality.as_str() {
            "Low" => 0.0,
            "Medium" => 1.0,
            "High" => 2.0,
            "Ultra" => 3.0,
            _ => 2.0,
        };
        if vc.ssr_temporal_upsampling {
            ssr_temporal = 1.0;
        }
    }
    lighting_uniform.ssr_active = ssr_active;
    lighting_uniform.ssr_quality = ssr_quality;
    lighting_uniform.ssr_temporal_upsampling = ssr_temporal;
}

impl crate::render::Renderer {
    /// Write the frame's lighting uniform and stage its point and spot lights (#434;
    /// uploaded once the shadow atlas has placed them, #468), and start the frame
    /// counters (#433) with the light counts.
    pub(super) fn upload_lighting(&mut self, scene: &Scene) {
        let lighting_uniform = self.build_lighting_uniform(scene);
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::bytes_of(&lighting_uniform),
        );
        self.clusters.stage(crate::render::clusters::local_lights(
            scene,
            self.static_capture,
        ));
        self.begin_counters(scene);
    }

    /// Builds the per-frame lighting uniform from the scene's lights and SSR settings,
    /// and the resident reflection probes' boxes (#1206). `sky_textured` is set only
    /// when a panorama is loaded (#718), else the shader reflects the procedural sky the
    /// sky pass draws.
    fn build_lighting_uniform(&self, scene: &Scene) -> LightingUniform {
        let mut lighting_uniform = default_lighting_uniform(scene);
        apply_scene_lights(&mut lighting_uniform, scene, self.static_capture);
        apply_ssr_settings(&mut lighting_uniform, scene);
        lighting_uniform.refl_probes = *self.reflection_probes.uniforms();
        if self.skybox_texture.is_some() {
            lighting_uniform.sky_textured = 1.0;
        }
        lighting_uniform
    }
}

#[cfg(test)]
#[path = "lighting_tests.rs"]
mod lighting_tests;
