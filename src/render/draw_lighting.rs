//! Construction of the per-frame `LightingUniform` from a scene's lights and SSR
//! (Visual Correction) settings. Free builders split out of `draw.rs` to keep each
//! file under the size cap; `Renderer::build_lighting_uniform` orchestrates them
//! (behavior unchanged).

use glam::Vec3;

use super::{
    AmbientLightUniform, DirectionalLightUniform, LightingUniform, PointLightUniform,
    SpotlightUniform,
};
use crate::scene::{LightType, Scene};

/// The base lighting uniform before scene lights/SSR are scanned in.
pub(super) fn default_lighting_uniform(scene: &Scene) -> LightingUniform {
    LightingUniform {
        ambient: AmbientLightUniform {
            color: scene.ambient_color.to_array(),
            intensity: scene.ambient_intensity,
        },
        dir_light: DirectionalLightUniform {
            direction: [0.0, -1.0, 0.0],
            _pad1: 0.0,
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            _pad2: [0.0; 4],
        },
        point_lights: [PointLightUniform {
            position: [0.0, 0.0, 0.0],
            _pad1: 0.0,
            color: [0.0, 0.0, 0.0],
            intensity: 0.0,
            range: 0.0,
            _pad2: [0.0; 3],
        }; 4],
        spot_light: SpotlightUniform {
            position: [0.0, 0.0, 0.0],
            _pad1: 0.0,
            direction: [0.0, 0.0, 0.0],
            _pad2: 0.0,
            color: [0.0, 0.0, 0.0],
            intensity: 0.0,
            range: 0.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            _pad3: 0.0,
        },
        num_point_lights: 0,
        ssr_active: 0.0,
        ssr_quality: 0.0,
        ssr_temporal_upsampling: 0.0,
        refl_active: 0.0,
        refl_has_cubemap: 0.0,
        _refl_pad: [0.0; 2],
        refl_center: [0.0; 4],
        refl_box_min: [0.0; 4],
        refl_box_max: [0.0; 4],
    }
}

/// Select the active reflection probe for this frame and write its box + centre into
/// the lighting uniform (#244). The probe whose box contains the camera and is nearest
/// to it wins; with none, `refl_active` stays 0 and the shader keeps the plain skybox
/// reflection. The selection mirrors `ReflectionProbeSet::select` (unit-tested there).
pub(super) fn apply_reflection_probe(
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

/// Populate the dynamic light slots from active light entities (point lights capped
/// at the 4-slot budget).
pub(super) fn apply_scene_lights(lighting_uniform: &mut LightingUniform, scene: &Scene) {
    let mut pt_idx = 0;
    for entity in scene.iter() {
        if !entity.active {
            continue;
        }
        if let Some(light) = &entity.light {
            match light.light_type {
                LightType::Ambient => {
                    lighting_uniform.ambient = AmbientLightUniform {
                        color: light.color.to_array(),
                        intensity: light.intensity,
                    };
                }
                LightType::Directional => {
                    let dir = entity.transform.rotation * Vec3::NEG_Z;
                    lighting_uniform.dir_light = DirectionalLightUniform {
                        direction: dir.to_array(),
                        _pad1: 0.0,
                        color: light.color.to_array(),
                        intensity: light.intensity,
                        _pad2: [0.0; 4],
                    };
                }
                LightType::Point => {
                    if pt_idx < 4 {
                        lighting_uniform.point_lights[pt_idx] = PointLightUniform {
                            position: entity.transform.position.to_array(),
                            _pad1: 0.0,
                            color: light.color.to_array(),
                            intensity: light.intensity,
                            range: light.range,
                            _pad2: [0.0; 3],
                        };
                        pt_idx += 1;
                    }
                }
                LightType::Spotlight => {
                    lighting_uniform.spot_light = spotlight_uniform(&entity, light);
                }
            }
        }
    }
    lighting_uniform.num_point_lights = pt_idx as u32;
}

/// Build the spotlight uniform, baking cone half-angles into their cosines.
pub(super) fn spotlight_uniform(
    entity: &crate::components::Entity,
    light: &crate::components::LightComponent,
) -> SpotlightUniform {
    let dir = entity.transform.rotation * Vec3::NEG_Z;
    SpotlightUniform {
        position: entity.transform.position.to_array(),
        _pad1: 0.0,
        direction: dir.to_array(),
        _pad2: 0.0,
        color: light.color.to_array(),
        intensity: light.intensity,
        range: light.range,
        inner_cone: light.inner_cone.to_radians().cos(),
        outer_cone: light.outer_cone.to_radians().cos(),
        _pad3: 0.0,
    }
}

/// Fold active Visual Correction (SSR) components into the uniform; last one wins.
pub(super) fn apply_ssr_settings(lighting_uniform: &mut LightingUniform, scene: &Scene) {
    let mut ssr_active = 0.0;
    let mut ssr_quality = 2.0; // High default
    let mut ssr_temporal = 0.0;
    for entity in scene.iter() {
        if !entity.active {
            continue;
        }
        if let Some(vc) = &entity.visual_correction {
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
    }
    lighting_uniform.ssr_active = ssr_active;
    lighting_uniform.ssr_quality = ssr_quality;
    lighting_uniform.ssr_temporal_upsampling = ssr_temporal;
}
