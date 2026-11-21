use std::ops::RangeInclusive;

use glam::{Mat4, Vec3};

use crate::gpu::lightmaps::Lightmaps;
use crate::gpu::uniforms::{DetailUniform, MappingUniform, ObjectData, SurfaceUniform};
use crate::postfx::effects::ssr;
use crate::MaterialData;
use rusty_components::{MaterialAsset, RefractionModel, RenderFace, RenderMode, UvMapping};
use rusty_scene::lighting::lightmap_set::bakes_mesh;
use rusty_scene::rules::material::{BLEND_SHARPNESS_RANGE, MAP_COORD_RANGE, WORLD_SIZE_RANGE};
use rusty_scene::Scene;

pub(crate) fn material_data(lit: bool, material: Option<&MaterialAsset>) -> MaterialData {
    let is_lit = u32::from(lit);
    let color_tint = material_color_tint(material);

    let (metallic, roughness) = material.map_or((0.0, 0.5), |m| (m.metallic, m.roughness));

    let use_texture = u32::from(material.is_some_and(|m| m.base_color_map.is_some()));
    let use_metallic_map = u32::from(material.is_some_and(|m| m.metallic_map.is_some()));
    let use_roughness_map = u32::from(material.is_some_and(|m| m.roughness_map.is_some()));
    let use_normal_map = u32::from(material.is_some_and(|m| m.normal_map.is_some()));
    let use_emissive_map = u32::from(material.is_some_and(|m| m.emissive_map.is_some()));

    let use_cutout = u32::from(material.is_some_and(|m| m.render_mode == RenderMode::Cutout));
    let alpha_cutoff = material.map_or(0.5, |m| m.alpha_cutoff);

    let receive_decals =
        u32::from(lit && material.is_none_or(|m| m.receive_decals && !m.is_transparent()));

    let emissive = material.map_or([0.0; 4], |m| {
        [m.emissive[0], m.emissive[1], m.emissive[2], 0.0]
    });

    let reflects = |m: &MaterialAsset| (m.dielectric_f0(), m.dielectric_f90());
    let (f0, f90) = material.map_or_else(|| reflects(&MaterialAsset::default()), reflects);
    MaterialData {
        color_tint,
        use_texture,
        is_lit,
        metallic,
        roughness,
        use_metallic_map,
        use_roughness_map,
        use_normal_map,
        use_emissive_map,
        emissive,
        use_cutout,
        alpha_cutoff,
        receive_decals,
        two_sided: u32::from(material.is_some_and(|m| m.render_face != RenderFace::Front)),
        surface: surface_uniform(lit, material),
        mapping: material.map_or_else(MappingUniform::default, mapping_uniform),
        normal_variance: [0; 4],
        dielectric_reflectance: [f0[0], f0[1], f0[2], f90],
        detail: material.map_or_else(DetailUniform::default, super::detail::detail_uniform),
        level_maps: super::map_slots::level_maps_uniform(material),
    }
}

pub(crate) fn mapping_uniform(c: &MaterialAsset) -> MappingUniform {
    let within = |v: f32, r: &RangeInclusive<f32>| v.clamp(*r.start(), *r.end());
    let [sx, sy] = c.tiling.map(|v| within(v, &MAP_COORD_RANGE));
    let [ox, oy] = c.offset.map(|v| within(v, &MAP_COORD_RANGE));
    let oy = match c.uv_mapping {
        UvMapping::Uv0 => 1.0 - sy - oy,
        UvMapping::Planar | UvMapping::Triplanar => oy,
    };
    MappingUniform {
        uv_st: [sx, sy, ox, oy],
        uv_mapping: c.uv_mapping as u32,
        world_scale: 1.0 / within(c.world_size, &WORLD_SIZE_RANGE),
        blend_sharpness: within(c.blend_sharpness, &BLEND_SHARPNESS_RANGE),
        _pad: 0,
    }
}

pub(crate) fn surface_uniform(lit: bool, material: Option<&MaterialAsset>) -> SurfaceUniform {
    let Some(m) = material else {
        return SurfaceUniform {
            receive_shadows: 1,
            receive_ssr: u32::from(lit),
            ..SurfaceUniform::default()
        };
    };
    let glass = m.is_transparent();
    let reads_opaque = lit && m.reads_opaque();
    let (refraction_model, thickness) = match m.refraction_model {
        RefractionModel::None => (0, 0.0),
        RefractionModel::Planar => (1, m.thickness),
        RefractionModel::Thin => (2, REFRACTION_THIN_DISTANCE),
    };
    SurfaceUniform {
        blend_mode: if glass { m.blend_mode as u32 } else { 0 },
        premultiply_diffuse: u32::from(lit && glass && preserves_specular(m)),
        receive_shadows: u32::from(m.receive_shadows),
        refraction_model: if reads_opaque { refraction_model } else { 0 },
        ior: m.ior.max(1.0),
        thickness,
        uses_opaque: u32::from(reads_opaque),
        depth_postpass: u32::from(glass && m.depth_postpass),
        transmission: transmission(m.transmission_color, m.transmission),
        absorption: absorption(m.transmittance_color, m.absorption_distance, m.thickness),
        receive_ssr: u32::from(
            lit && if glass {
                ssr::transparent::traces(m)
            } else {
                m.receives_ssr()
            },
        ),
        reactive: if glass { m.reactive.gpu() } else { 0.0 },
        writes_motion: u32::from(glass && m.writes_motion),
        _pad: 0,
    }
}

fn transmission([r, g, b]: [f32; 3], amount: f32) -> [f32; 4] {
    [r * amount, g * amount, b * amount, amount]
}

const REFRACTION_THIN_DISTANCE: f32 = 0.005;

fn absorption(color: [f32; 3], distance: f32, thickness: f32) -> [f32; 4] {
    let k = |c: f32| -(c + 1e-4).ln().min(0.0) / distance.max(1e-4);
    [k(color[0]), k(color[1]), k(color[2]), thickness]
}

pub(crate) fn preserves_specular(m: &MaterialAsset) -> bool {
    m.preserve_specular && m.blend_mode.preserves_specular()
}

pub(crate) fn solid_object(
    scene: &Scene,
    id: u32,
    (model_matrix, centre): (Mat4, Vec3),
    capture: &crate::ibl::cubemap_capture::CaptureMode,
    lightmaps: &Lightmaps,
) -> ObjectData {
    let (use_sh, sh) = entity_probe_sh(scene, id, centre, capture);
    let lightmap = scene.lightmaps.get(id).filter(|_| lightmaps.resident());
    ObjectData {
        model_matrix: model_matrix.to_cols_array(),
        use_sh,
        lightmap_page: lightmap.map_or(0, |l| l.page + 1),
        lightmap_directional: u32::from(lightmap.is_some() && lightmaps.directional()),
        lightmap_st: lightmap.map_or([0.0; 4], |l| l.scale_offset),
        in_bake: u32::from(scene.lightmaps.holds_bake() && bakes_mesh(scene, id)),
        sh,
        occlusion: probe_occlusion_at(scene, centre),
    }
}

fn material_color_tint(material: Option<&MaterialAsset>) -> [f32; 4] {
    if let Some(mat) = material {
        let a = if mat.is_transparent() { mat.alpha } else { 1.0 };
        return [mat.base_color[0], mat.base_color[1], mat.base_color[2], a];
    }
    [1.0, 1.0, 1.0, 1.0]
}

fn entity_probe_sh(
    scene: &Scene,
    id: u32,
    centre: Vec3,
    capture: &crate::ibl::cubemap_capture::CaptureMode,
) -> (u32, [[f32; 4]; 9]) {
    let probe_lit =
        || !capture.static_only && scene.world.mesh(id).is_some_and(|m| m.reads_probes());
    if scene.world.is_static(id) && !capture.probe_bounce && !probe_lit() {
        return (0, [[0.0; 4]; 9]);
    }
    probe_sh_at(scene, centre)
}

pub(crate) fn probe_sh_at(scene: &Scene, position: glam::Vec3) -> (u32, [[f32; 4]; 9]) {
    match scene.probes.sample(position) {
        Some(probe) => {
            let mut sh = [[0.0f32; 4]; 9];
            for (i, c) in probe.coeffs.iter().enumerate() {
                sh[i] = [c[0], c[1], c[2], 0.0];
            }
            (1, sh)
        }
        None => (0, [[0.0; 4]; 9]),
    }
}

pub(crate) fn probe_occlusion_at(scene: &Scene, position: glam::Vec3) -> [f32; 4] {
    if scene.lightmaps.channels.is_empty() {
        return [1.0; 4];
    }
    let o = scene
        .probes
        .sample_occlusion(position)
        .unwrap_or(glam::Vec4::ONE);
    match o.is_finite() {
        true => o.clamp(glam::Vec4::ZERO, glam::Vec4::ONE).to_array(),
        false => [1.0; 4],
    }
}

#[cfg(test)]
#[path = "uniforms_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "probe_sh_tests.rs"]
mod probe_sh_tests;

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod mapping_tests;

#[cfg(test)]
#[path = "temporal_tests.rs"]
mod temporal_tests;
