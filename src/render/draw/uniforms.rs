//! Per-entity forward-pass data builders: the tint/PBR/cutout `MaterialData` record a
//! solid mesh's draw reads (#1616), and the `InstanceData` (world matrix + light-probe
//! SH) it adds to that draw's instance array (#470).
//! Split out of `draw_resources` to keep that file under the size cap; behaviour
//! unchanged. Free functions (no GPU state), called from `sync_solid_resource`.

use std::ops::RangeInclusive;

use glam::Mat4;

use crate::components::{MaterialAsset, RefractionModel, RenderFace, RenderMode};
use crate::render::gpu::lightmaps::Lightmaps;
use crate::render::gpu::uniforms::{MappingUniform, SurfaceUniform};
use crate::render::postfx::effects::ssr;
use crate::render::{InstanceData, MaterialData};
use crate::scene::authoring::material::{BLEND_SHARPNESS_RANGE, MAP_COORD_RANGE, WORLD_SIZE_RANGE};
use crate::scene::Scene;

/// Compute the material record (tint, lit flag, PBR params, cutout) for a solid
/// mesh. It carries no transform — that is per instance — so every entity sharing a
/// material produces the same record and can share a draw call (#470). Tint is driven by components only — never by entity name. A game colours its
/// entities via its referenced material's `base_color`; the engine carries no
/// per-name colour assumptions. `material` is the entity's resolved library material
/// (`None` when it references none).
pub(crate) fn solid_material_data(
    scene: &Scene,
    id: u32,
    material: Option<&MaterialAsset>,
) -> MaterialData {
    // A light's own gizmo mesh is drawn unlit.
    material_data(!scene.world.has_light(id), material)
}

/// The material record for `material`, lit or not — shared by entity solids and
/// mesh particles (#440), which have no entity of their own.
pub(crate) fn material_data(lit: bool, material: Option<&MaterialAsset>) -> MaterialData {
    let is_lit = u32::from(lit);
    let color_tint = material_color_tint(material);

    let (metallic, roughness) = match material {
        Some(mat) => (mat.metallic, mat.roughness),
        None => (0.0, 0.5),
    };

    let use_texture = u32::from(material.is_some_and(|m| m.base_color_map.is_some()));
    let use_metallic_map = u32::from(material.is_some_and(|m| m.metallic_map.is_some()));
    let use_roughness_map = u32::from(material.is_some_and(|m| m.roughness_map.is_some()));
    let use_normal_map = u32::from(material.is_some_and(|m| m.normal_map.is_some()));
    let use_emissive_map = u32::from(material.is_some_and(|m| m.emissive_map.is_some()));

    // Cutout alpha-test (#242): only a Cutout material discards; Opaque/Transparent
    // leave the flag clear so the shader's discard branch is skipped.
    let use_cutout = u32::from(material.is_some_and(|m| m.render_mode == RenderMode::Cutout));
    let alpha_cutoff = material.map_or(0.5, |m| m.alpha_cutoff);

    // Decals (#638) land on lit surfaces whose material takes them, never on glass.
    let receive_decals =
        u32::from(lit && material.is_none_or(|m| m.receive_decals && !m.is_transparent()));

    // Flat emissive factor (#222), 4th lane unused. Defaults to black with no material.
    let emissive = match material {
        Some(mat) => [mat.emissive[0], mat.emissive[1], mat.emissive[2], 0.0],
        None => [0.0, 0.0, 0.0, 0.0],
    };

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
    }
}

/// Where material `c` reads its maps (#1573): its tiling and offset as one
/// scale/offset, its mapping by index, and its world size as repeats per metre.
/// Each value is clamped to its authoring row's range, since a hand-written scene
/// skips the row's own clamp and a sharpness past it overflows the shader's blend.
pub(crate) fn mapping_uniform(c: &MaterialAsset) -> MappingUniform {
    let within = |v: f32, r: &RangeInclusive<f32>| v.clamp(*r.start(), *r.end());
    let [sx, sy] = c.tiling.map(|v| within(v, &MAP_COORD_RANGE));
    let [ox, oy] = c.offset.map(|v| within(v, &MAP_COORD_RANGE));
    MappingUniform {
        uv_st: [sx, sy, ox, oy],
        uv_mapping: c.uv_mapping as u32,
        world_scale: 1.0 / within(c.world_size, &WORLD_SIZE_RANGE),
        blend_sharpness: within(c.blend_sharpness, &BLEND_SHARPNESS_RANGE),
        _pad: 0,
    }
}

/// The transparent-surface terms of `material` (#1523). Blending and preserve
/// specular apply to a `Transparent` material alone; a draw with no material
/// receives shadows. An unlit draw (`lit` false: a light's own mesh) returns its
/// colour before any lighting, so it neither premultiplies nor reads the opaque
/// copy: it blends straight, as before #1523.
pub(crate) fn surface_uniform(lit: bool, material: Option<&MaterialAsset>) -> SurfaceUniform {
    let Some(m) = material else {
        return SurfaceUniform {
            receive_shadows: 1,
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
        // A leftover blend mode on an Opaque or Cutout material blends nothing.
        blend_mode: if glass { m.blend_mode as u32 } else { 0 },
        premultiply_diffuse: u32::from(lit && glass && preserves_specular(m)),
        receive_shadows: u32::from(m.receive_shadows),
        refraction_model: if reads_opaque { refraction_model } else { 0 },
        ior: m.ior,
        thickness,
        uses_opaque: u32::from(reads_opaque),
        depth_postpass: u32::from(glass && m.depth_postpass),
        transmission: transmission(m.transmission_color, m.transmission),
        absorption: absorption(m.transmittance_color, m.absorption_distance, m.thickness),
        receive_ssr: u32::from(lit && glass && ssr::transparent::traces(m)),
        _pad: [0; 3],
    }
}

/// Thin translucency's lane: the colour times the amount, then the amount.
fn transmission([r, g, b]: [f32; 3], amount: f32) -> [f32; 4] {
    [r * amount, g * amount, b * amount, amount]
}

/// HDRP's thin refraction slab (`Lit.hlsl`, `REFRACTION_THIN_DISTANCE`): 5 mm.
const REFRACTION_THIN_DISTANCE: f32 = 0.005;

/// The absorption coefficient per metre that leaves `color` after `distance`
/// metres, HDRP's `TransmittanceColorAtDistanceToAbsorption`:
/// `-ln(color) / distance`, with its epsilon on both. The fourth lane is the
/// surface's `thickness`, the depth it absorbs over where the sky is behind it, as
/// HDRP's thickness drives its absorption.
fn absorption(color: [f32; 3], distance: f32, thickness: f32) -> [f32; 4] {
    let k = |c: f32| -(c + 1e-4).ln().min(0.0) / distance.max(1e-4);
    [k(color[0]), k(color[1]), k(color[2]), thickness]
}

/// Whether URP's *Preserve Specular Lighting* is in effect for `m`: on, and under a
/// blend mode that keeps it (`BlendMode::preserves_specular`).
pub(crate) fn preserves_specular(m: &MaterialAsset) -> bool {
    m.preserve_specular && m.blend_mode.preserves_specular()
}

/// One solid's instance: its world matrix, the SH its ambient term reads when a light
/// probe covers it (see [`entity_probe_sh`]), and its baked lightmap when it has one
/// and `lightmaps` has its pages bound (#438), directional when their direction
/// pages are bound too (#810).
pub(crate) fn solid_instance(
    scene: &Scene,
    id: u32,
    model_matrix: Mat4,
    light_static_from_probes: bool,
    lightmaps: &Lightmaps,
) -> InstanceData {
    let (use_sh, sh) = entity_probe_sh(scene, id, model_matrix, light_static_from_probes);
    let lightmap = scene.lightmaps.get(id).filter(|_| lightmaps.resident());
    InstanceData {
        model_matrix: model_matrix.to_cols_array(),
        use_sh,
        lightmap_page: lightmap.map_or(0, |l| l.page + 1),
        lightmap_directional: u32::from(lightmap.is_some() && lightmaps.directional()),
        // The draw sets its probe from its world bounds (`solid_draw_item`, #1206).
        reflection_probe: 0,
        lightmap_st: lightmap.map_or([0.0; 4], |l| l.scale_offset),
        sh,
        // No history until `motion::track` gives it one (#1435).
        ..InstanceData::IDENTITY
    }
}

/// The RGBA tint lane for a solid mesh. RGB comes from the material's `base_color`
/// (or white when the entity references no material). The alpha lane is 1.0 for
/// Opaque/Cutout — keeping the opaque path byte-for-byte unchanged — and the
/// material's `alpha` only for a Transparent material, where it is the blend factor.
fn material_color_tint(material: Option<&MaterialAsset>) -> [f32; 4] {
    if let Some(mat) = material {
        let a = if mat.is_transparent() { mat.alpha } else { 1.0 };
        return [mat.base_color[0], mat.base_color[1], mat.base_color[2], a];
    }
    [1.0, 1.0, 1.0, 1.0]
}

/// Resolve the light-probe SH for one entity (#240): the scene's probe field sampled
/// (trilinear) at the entity's world position, flattened to the `vec4`-padded GPU
/// layout. Only NON-STATIC objects opt in — static geometry keeps the flat ambient
/// term (and is the bake's target, not its consumer). Returns `(use_sh, coeffs)`;
/// `use_sh == 0` with zeroed coeffs when the entity is static or no probe covers it.
///
/// `light_static_from_probes` overrides the "static ⇒ no probe SH" rule (#285): the
/// multi-bounce probe bake sets it so static surfaces *do* sample the in-progress
/// probe field during a bounce-≥2 capture, feeding the previous bounce's indirect
/// light back into the scene. It is off everywhere else (runtime shading, bounce 1).
fn entity_probe_sh(
    scene: &Scene,
    id: u32,
    model_matrix: Mat4,
    light_static_from_probes: bool,
) -> (u32, [[f32; 4]; 9]) {
    if scene.world.is_static(id) && !light_static_from_probes {
        return (0, [[0.0; 4]; 9]);
    }
    probe_sh_at(scene, model_matrix.w_axis.truncate())
}

/// The light-probe SH at `position` in the GPU layout, `(use_sh, coeffs)` — zeroed
/// with `use_sh == 0` when no probe covers it (#240; mesh particles, #440).
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

#[cfg(test)]
#[path = "uniforms_tests.rs"]
mod tests;
