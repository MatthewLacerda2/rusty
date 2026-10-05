//! Construction of the per-frame `LightingUniform` from a scene's lights and SSR
//! (Visual Correction) settings. Free builders split out of `draw.rs` to keep each
//! file under the size cap; `Renderer::build_lighting_uniform` orchestrates them
//! (behavior unchanged).

use glam::Vec3;

use crate::render::gpu::uniforms::MAX_DIRECTIONAL_LIGHTS;
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
        refl_active: 0.0,
        refl_has_cubemap: 0.0,
        sky_textured: 0.0,
        _refl_pad: 0.0,
        refl_center: [0.0; 4],
        refl_box_min: [0.0; 4],
        refl_box_max: [0.0; 4],
    }
}

/// Select the active reflection probe for this frame and write its box + centre into
/// the lighting uniform (#244). The probe whose box contains the camera and is nearest
/// to it wins; with none, `refl_active` stays 0 and the shader keeps the plain skybox
/// reflection. The selection mirrors `ReflectionProbeSet::select` (unit-tested there).
pub(crate) fn apply_reflection_probe(
    lighting_uniform: &mut LightingUniform,
    scene: &Scene,
    camera_pos: Vec3,
) {
    if let Some(probe) = scene.reflection_probes.select(camera_pos) {
        lighting_uniform.refl_active = 1.0;
        lighting_uniform.refl_center = probe.position.extend(0.0).to_array();
        lighting_uniform.refl_box_min = probe.box_min.extend(0.0).to_array();
        lighting_uniform.refl_box_max = probe.box_max.extend(0.0).to_array();
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
        if !scene.world.is_active(id) {
            continue;
        }
        let light = scene.world.light(id).expect("id came from ids_with_light");
        let transform = scene.world.transform(id).expect("mandatory Transform");
        match light.light_type {
            LightType::Ambient => {
                lighting_uniform.ambient = AmbientLightUniform {
                    color: light.color.to_array(),
                    intensity: light.intensity,
                };
            }
            LightType::Directional if light.mode.renders_live(capture) => {
                dirs.push(DirectionalLightUniform {
                    direction: (transform.rotation * Vec3::NEG_Z).to_array(),
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

/// Fold active Visual Correction (SSR) components into the uniform; last one wins.
pub(crate) fn apply_ssr_settings(lighting_uniform: &mut LightingUniform, scene: &Scene) {
    let mut ssr_active = 0.0;
    let mut ssr_quality = 2.0; // High default
    let mut ssr_temporal = 0.0;
    for id in scene.world.ids_with_visual_correction() {
        if !scene.world.is_active(id) {
            continue;
        }
        let vc = scene
            .world
            .visual_correction(id)
            .expect("id came from ids_with_visual_correction");
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
    pub(super) fn upload_lighting(&mut self, scene: &Scene, camera_pos: Vec3) {
        let lighting_uniform = self.build_lighting_uniform(scene, camera_pos);
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

    /// Builds the per-frame lighting uniform from the scene's lights and SSR settings.
    /// `refl_has_cubemap` is set only when a baked cube is actually loaded for the active
    /// probe (`self.reflection_cube`), so the shader never samples the black fallback cube;
    /// likewise `sky_textured` only when a panorama is loaded (#718), else the shader
    /// reflects the procedural sky the sky pass draws.
    fn build_lighting_uniform(&self, scene: &Scene, camera_pos: Vec3) -> LightingUniform {
        let mut lighting_uniform = default_lighting_uniform(scene);
        apply_scene_lights(&mut lighting_uniform, scene, self.static_capture);
        apply_ssr_settings(&mut lighting_uniform, scene);
        apply_reflection_probe(&mut lighting_uniform, scene, camera_pos);
        if self.reflection_cube.is_some() {
            lighting_uniform.refl_has_cubemap = 1.0;
        }
        if self.skybox_texture.is_some() {
            lighting_uniform.sky_textured = 1.0;
        }
        lighting_uniform
    }
}
