//! Construction of the per-frame `LightingUniform` from a scene's lights. Free
//! builders split out of `draw.rs` to keep each file under the size cap;
//! `Renderer::build_lighting_uniform` orchestrates them.

use glam::Vec3;

use crate::render::gpu::uniforms::{ProceduralSkyUniform, MAX_DIRECTIONAL_LIGHTS};
use crate::render::ibl::reflection_probes::MAX_REFLECTION_PROBES;
use crate::render::{AmbientLightUniform, DirectionalLightUniform, LightingUniform};
use crate::scene::{LightType, Scene};

/// The base lighting uniform, with the scene's ambient, before its directional lights
/// are scanned in.
pub(crate) fn default_lighting_uniform(scene: &Scene) -> LightingUniform {
    LightingUniform {
        ambient: scene_ambient(scene),
        sky: ProceduralSkyUniform::from_settings(&scene.sky),
        dir_lights: [bytemuck::Zeroable::zeroed(); MAX_DIRECTIONAL_LIGHTS],
        num_dir_lights: 0,
        sky_textured: 0.0,
        _sky_pad: [0.0; 2],
        refl_probes: [Default::default(); MAX_REFLECTION_PROBES],
    }
}

/// The frame's ambient: `Scene::drawn_ambient`, the rule the Scene Settings panel
/// shares.
pub(crate) fn scene_ambient(scene: &Scene) -> AmbientLightUniform {
    let drawn = scene.drawn_ambient();
    let [sky, equator, ground] = drawn.gradient.unwrap_or([Vec3::ZERO; 3]);
    AmbientLightUniform {
        color: drawn.color.to_array(),
        intensity: drawn.intensity,
        gradient_sky: sky.to_array(),
        gradient: f32::from(u8::from(drawn.gradient.is_some())),
        gradient_equator: equator.to_array(),
        _pad_equator: 0.0,
        gradient_ground: ground.to_array(),
        _pad_ground: 0.0,
    }
}

/// Fill the directional slots from the active light entities (the ambient is
/// [`scene_ambient`]'s). Directional lights fill [`MAX_DIRECTIONAL_LIGHTS`] slots in entity
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
            LightType::Ambient
            | LightType::Directional
            | LightType::Point
            | LightType::Spotlight => {}
        }
    }
    if let Some(sun) = dirs.pop() {
        dirs.insert(0, sun);
    }
    dirs.truncate(MAX_DIRECTIONAL_LIGHTS);
    lighting_uniform.dir_lights[..dirs.len()].copy_from_slice(&dirs);
    lighting_uniform.num_dir_lights = dirs.len() as u32;
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
            self.capture.static_only,
        ));
        self.begin_counters(scene);
    }

    /// Builds the per-frame lighting uniform from the scene's lights,
    /// and the resident reflection probes' boxes (#1206). `sky_textured` is set only
    /// when a panorama is loaded (#718), else the shader reflects the procedural sky the
    /// sky pass draws.
    fn build_lighting_uniform(&self, scene: &Scene) -> LightingUniform {
        let mut lighting_uniform = default_lighting_uniform(scene);
        apply_scene_lights(&mut lighting_uniform, scene, self.capture.static_only);
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
