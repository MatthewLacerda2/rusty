//! Construction of the per-frame `LightingUniform` from a scene's lights. Free
//! builders split out of `draw.rs` to keep each file under the size cap;
//! `Renderer::build_lighting_uniform` orchestrates them.

use glam::Vec3;

use crate::gpu::uniforms::{ProceduralSkyUniform, MAX_DIRECTIONAL_LIGHTS};
use crate::ibl::cubemap_capture::CaptureMode;
use crate::ibl::reflection_probes::MAX_REFLECTION_PROBES;
use crate::{AmbientLightUniform, DirectionalLightUniform, LightingUniform};
use rusty_core::settings::quality::ShadowmaskMode;
use rusty_scene::lighting::{DiffuseModel, MixedLightingMode};
use rusty_scene::{LightType, Scene};

/// The base lighting uniform, with the scene's ambient, before its directional lights
/// are scanned in.
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

/// The frame's ambient: `Scene::drawn_ambient`, the rule the Scene Settings panel
/// shares.
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

/// Fill the directional slots from the active light entities (the ambient is
/// [`scene_ambient`]'s). Directional lights fill [`MAX_DIRECTIONAL_LIGHTS`] slots in entity
/// order, except that the last one (the shadow pass's sun) always takes slot 0.
/// Point and spot lights go to the cluster lights instead (#434). Only the
/// directional lights [`LightMode::renders_live`] keeps for a frame or a bake
/// `capture` are uploaded (#809); the ambient ignores modes.
///
/// [`LightMode::renders_live`]: rusty_components::LightMode::renders_live
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
            LightType::Directional if scene.light_mode(id, &light).renders_live(capture) => {
                dirs.push(DirectionalLightUniform {
                    direction: scene.world.transform_direction(id, Vec3::NEG_Z).to_array(),
                    _pad1: 0.0,
                    color: light.color.to_array(),
                    intensity: light.intensity,
                    // `Baked` (#438), only ever uploaded in a bake capture: a
                    // lightmapped surface skips it, its lightmap holds it.
                    baked: f32::from(u8::from(scene.light_mode(id, &light).bakes_direct())),
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

/// The lighting uniform's `shadowmask` (#1695): 0 off, 1 `Shadowmask`, 2
/// `DistanceShadowmask`. Off unless the scene is on `Shadowmask` and its bake gave
/// a light a channel: a `BakedIndirect` scene draws as it always did.
pub(crate) fn shadowmask_mode(scene: &Scene, quality: ShadowmaskMode) -> f32 {
    let on = scene.lighting_settings.mixed_lighting_mode == MixedLightingMode::Shadowmask;
    match (on && !scene.lightmaps.channels.is_empty(), quality) {
        (false, _) => 0.0,
        (true, ShadowmaskMode::Shadowmask) => 1.0,
        (true, ShadowmaskMode::DistanceShadowmask) => 2.0,
    }
}

impl crate::Renderer {
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
        self.clusters.stage(crate::clusters::local_lights(
            scene,
            self.capture.static_only,
        ));
        self.begin_counters(scene);
    }

    /// Builds the per-frame lighting uniform from the scene's lights,
    /// and the resident reflection probes' boxes (#1206). `sky_textured` is set only
    /// when a panorama is loaded (#718), else the shader reflects the procedural sky the
    /// sky pass draws. A light-probe capture reflects nothing (#1668): Unity's
    /// light-probe bake leaves specular out, so neither the sky nor a reflection probe
    /// reaches a probe through a glossy static surface. No bake capture normalises a
    /// probe's reflection by its stored irradiance (#1697): a reflection capture's
    /// glossy static surfaces reflect the resident probes at their baked strength,
    /// so what a capture sees never depends on another probe's half-finished or
    /// stale normalisation, nor on the order probes bake in.
    fn build_lighting_uniform(&self, scene: &Scene) -> LightingUniform {
        let mut lighting_uniform = default_lighting_uniform(scene);
        apply_scene_lights(&mut lighting_uniform, scene, self.capture.static_only);
        // A bake capture draws a `Mixed` light as `BakedIndirect` does (#1695): its
        // realtime maps hold every static caster, so the capture needs no mask.
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

/// What a bake capture turns off in the lighting uniform (`capture`; a frame's
/// default mode changes nothing). Every bake capture leaves the probes' normalisation
/// out (#1697) and the directional lightmap's specular lobe (#1695: it depends on the
/// view, and a probe is seen from everywhere); a light-probe capture reflects nothing
/// (#1668).
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
