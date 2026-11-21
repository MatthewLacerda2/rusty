use crate::gpu::ops::CountedQueue as _;
use glam::Vec3;

use crate::gpu::uniforms::{ProceduralSkyUniform, MAX_DIRECTIONAL_LIGHTS};
use crate::ibl::cubemap_capture::CaptureMode;
use crate::ibl::reflection_probes::MAX_REFLECTION_PROBES;
use crate::{AmbientLightUniform, DirectionalLightUniform, LightingUniform};
use rusty_core::settings::quality::ShadowmaskMode;
use rusty_scene::lighting::{DiffuseModel, MixedLightingMode};
use rusty_scene::{LightType, Scene};

pub(crate) fn default_lighting_uniform(scene: &Scene) -> LightingUniform {
    LightingUniform {
        ambient: scene_ambient(scene),
        sky: ProceduralSkyUniform::from_settings(&scene.sky),
        dir_lights: [bytemuck::Zeroable::zeroed(); MAX_DIRECTIONAL_LIGHTS],
        num_dir_lights: 0,
        sky_textured: 0.0,
        env_reflection: 1.0,
        diffuse_wrap: match scene.lighting_settings.diffuse_model {
            DiffuseModel::Lambert => 0.0,
            DiffuseModel::HalfLambert => 1.0,
        },
        baked_specular: 1.0,
        shadowmask: 0.0,
        masks_bound: 0.0,
        _pad_baked_specular: 0.0,
        refl_probes: [Default::default(); MAX_REFLECTION_PROBES],
    }
}

pub(crate) fn scene_ambient(scene: &Scene) -> AmbientLightUniform {
    let drawn = scene.drawn_ambient();
    let [sky, equator, ground] = drawn.gradient.unwrap_or([Vec3::ZERO; 3]);
    let mode = match (drawn.skybox, drawn.gradient) {
        (Some(_), _) => 2.0,
        (None, Some(_)) => 1.0,
        (None, None) => 0.0,
    };
    let mut sky_sh = [[0.0; 4]; 9];
    if let rusty_scene::AmbientRadiance::Sky(sh) = drawn.radiance() {
        for (out, c) in sky_sh.iter_mut().zip(sh.convolved_over_pi().coeffs) {
            *out = [c[0], c[1], c[2], 0.0];
        }
    }
    AmbientLightUniform {
        color: drawn.color.to_array(),
        intensity: drawn.intensity,
        gradient_sky: sky.to_array(),
        mode,
        gradient_equator: equator.to_array(),
        _pad_equator: 0.0,
        gradient_ground: ground.to_array(),
        _pad_ground: 0.0,
        sky_sh,
    }
}

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
        let mode = crate::clusters::drawn_mode(scene, id, &light);
        match light.light_type {
            LightType::Directional if mode.renders_live(capture) => {
                dirs.push(DirectionalLightUniform {
                    direction: scene.world.transform_direction(id, Vec3::NEG_Z).to_array(),
                    _pad1: 0.0,
                    color: light.color.to_array(),
                    intensity: light.intensity,
                    baked: f32::from(u8::from(mode.bakes_direct())),
                    bounce_baked: f32::from(u8::from(scene.lightmaps.baked_light(id))),
                    mask_channel: scene.mask_channel(id).map_or(-1.0, f32::from),
                    _pad2: 0.0,
                })
            }
            LightType::Ambient
            | LightType::Directional
            | LightType::Point
            | LightType::Spotlight
            | LightType::Area => {}
        }
    }
    if let Some(sun) = dirs.pop() {
        dirs.insert(0, sun);
    }
    dirs.truncate(MAX_DIRECTIONAL_LIGHTS);
    lighting_uniform.dir_lights[..dirs.len()].copy_from_slice(&dirs);
    lighting_uniform.num_dir_lights = dirs.len() as u32;
}

pub(crate) fn shadowmask_mode(scene: &Scene, quality: ShadowmaskMode) -> f32 {
    let on = scene.lighting_settings.mixed_lighting_mode == MixedLightingMode::Shadowmask;
    match (on && !scene.lightmaps.channels.is_empty(), quality) {
        (false, _) => 0.0,
        (true, ShadowmaskMode::Shadowmask) => 1.0,
        (true, ShadowmaskMode::DistanceShadowmask) => 2.0,
    }
}

impl crate::Renderer {
    pub(super) fn upload_lighting(&mut self, scene: &Scene) {
        let lighting_uniform = self.build_lighting_uniform(scene);
        self.queue.counted_write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::bytes_of(&lighting_uniform),
        );
        self.clusters.stage(crate::clusters::local_lights(
            scene,
            self.capture.static_only,
        ));
        self.begin_counters(scene);
    }

    fn build_lighting_uniform(&self, scene: &Scene) -> LightingUniform {
        let mut lighting_uniform = default_lighting_uniform(scene);
        apply_scene_lights(&mut lighting_uniform, scene, self.capture.static_only);
        if !self.capture.static_only {
            lighting_uniform.shadowmask = shadowmask_mode(scene, self.quality.shadowmask_mode);
            lighting_uniform.masks_bound = f32::from(u8::from(self.lightmaps.masked()));
        }
        lighting_uniform.refl_probes = *self.reflection_probes.uniforms();
        apply_capture(&mut lighting_uniform, self.capture);
        if self.skybox_texture.is_some() {
            lighting_uniform.sky_textured = 1.0;
        }
        lighting_uniform
    }
}

pub(crate) fn apply_capture(uniform: &mut LightingUniform, capture: CaptureMode) {
    if capture.static_only {
        uniform.baked_specular = 0.0;
        for probe in &mut uniform.refl_probes {
            probe.params[3] = 0.0;
        }
    }
    if capture.ambient_sky {
        uniform.env_reflection = 0.0;
    }
}

#[cfg(test)]
#[path = "lighting_tests.rs"]
mod lighting_tests;
