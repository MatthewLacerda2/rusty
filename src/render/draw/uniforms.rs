//! Per-entity forward-pass data builders: the tint/PBR/cutout `MaterialData` record a
//! solid mesh's draw reads (#1616), and the `ObjectData` (world matrix, light-probe
//! SH, lightmap) its kept object holds in the object table (#470, #1706).
//! Split out of `draw_resources` to keep that file under the size cap; behaviour
//! unchanged. Free functions (no GPU state), called from `sync_solid_resource`.

use std::ops::RangeInclusive;

use glam::{Mat4, Vec3};

use crate::components::{MaterialAsset, RefractionModel, RenderFace, RenderMode, UvMapping};
use crate::render::gpu::lightmaps::Lightmaps;
use crate::render::gpu::uniforms::{MappingUniform, ObjectData, SurfaceUniform};
use crate::render::postfx::effects::ssr;
use crate::render::MaterialData;
use crate::scene::authoring::material::{BLEND_SHARPNESS_RANGE, MAP_COORD_RANGE, WORLD_SIZE_RANGE};
use crate::scene::lighting::lightmap::BakeScene;
use crate::scene::Scene;

/// The material record (tint, lit flag, PBR params, cutout, surface and mapping
/// terms) for `material`, lit or not. It carries no transform and reads no entity,
/// so every draw of a material under one `render::scene::Variant` reads one record
/// (#470, #1706). Tint is driven by components only — never by entity name.
pub(crate) fn material_data(lit: bool, material: Option<&MaterialAsset>) -> MaterialData {
    let is_lit = u32::from(lit);
    let color_tint = material_color_tint(material);

    let (metallic, roughness) = material.map_or((0.0, 0.5), |m| (m.metallic, m.roughness));

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
    let emissive = material.map_or([0.0; 4], |m| {
        [m.emissive[0], m.emissive[1], m.emissive[2], 0.0]
    });

    // A draw with no material reflects as the default one (#1998).
    let f0 = material.map_or_else(
        || MaterialAsset::default().dielectric_f0(),
        |m| m.dielectric_f0(),
    );
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
        dielectric_reflectance: [f0[0], f0[1], f0[2], 0.0],
    }
}

/// Where material `c` reads its maps (#1573): its tiling and offset as one
/// scale/offset, its mapping by index, and its world size as repeats per metre.
/// Each value is clamped to its authoring row's range, since a hand-written scene
/// skips the row's own clamp and a sharpness past it overflows the shader's blend.
///
/// Tiling and Offset mean Unity's (#1851): Unity's `TRANSFORM_TEX` is
/// `uv * _ST.xy + _ST.zw` with v = 0 at the image's bottom, and UV0 here has v = 0 at
/// its top (glTF), so UV0's v lane is packed as `tiling.y * v + (1 - tiling.y -
/// offset.y)`, UnityGLTF's conversion of `KHR_texture_transform`: +y moves the
/// texture down the surface and Tiling scales from the image's bottom edge, as in
/// Unity. Planar and Triplanar project world coordinates, which HDRP transforms with
/// the plain `uv * _ST.xy + _ST.zw` (`ComputeLayerTexCoord`), so they are packed
/// as set: an offset moves the texture along the same world axis as in HDRP.
/// The material keeps the value set; only this packing converts it, and every
/// reader of `uv_st` (the forward maps, the cutout shadow casters) reads it here.
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

/// The transparent-surface terms of `material` (#1523). Blending and preserve
/// specular apply to a `Transparent` material alone; a draw with no material
/// receives shadows. An unlit draw (`lit` false: a light's own mesh) returns its
/// colour before any lighting, so it neither premultiplies nor reads the opaque
/// copy: it blends straight, as before #1523.
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
        // A translucent's lane says it traces (#1570); an opaque one's that its
        // pixels keep the SSR record, else take the probe only (#2004).
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

/// One solid's object record (#1706): its world matrix, the SH its ambient term reads when a light
/// probe covers it (see [`entity_probe_sh`]) and its probe shadowmask occlusion, both
/// sampled at `centre`, its world bounds' centre (#1994), and its baked lightmap when it has one
/// and `lightmaps` has its pages bound (#438), directional when their direction
/// pages are bound too (#810). `capture` is the renderer's [`CaptureMode`]: a bake
/// capture's static surfaces read probes only on a probe bounce.
///
/// [`CaptureMode`]: crate::render::ibl::cubemap_capture::CaptureMode
pub(crate) fn solid_object(
    scene: &Scene,
    id: u32,
    (model_matrix, centre): (Mat4, Vec3),
    capture: &crate::render::ibl::cubemap_capture::CaptureMode,
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
        in_bake: u32::from(scene.lightmaps.holds_bake() && BakeScene::takes_mesh(scene, id)),
        sh,
        occlusion: probe_occlusion_at(scene, centre),
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
/// (trilinear) at `centre`, its world bounds' centre (Unity's `Renderer.bounds.center`,
/// #1994; never its origin, which a mesh modelled in world coordinates leaves in
/// another room), flattened to the `vec4`-padded GPU
/// layout. Only NON-STATIC objects opt in — static geometry keeps the flat ambient
/// term (and is the bake's target, not its consumer) — and a static mesh at *Scale
/// In Lightmap* 0 (#1704), which the lightmap bake skips: as in Unity, it reads
/// light probes instead. Returns `(use_sh, coeffs)`; `use_sh == 0` with zeroed
/// coeffs when the entity is static (and lightmapped) or no probe covers it.
///
/// `capture.probe_bounce` overrides the "static ⇒ no probe SH" rule (#285): the
/// multi-bounce probe bake sets it so static surfaces *do* sample the in-progress
/// probe field during a bounce-≥2 capture, feeding the previous bounce's indirect
/// light back into the scene. It is off everywhere else (runtime shading, bounce 1).
/// A bake capture (`static_only`) treats a Scale In Lightmap 0 mesh as any static
/// one, so a bounce-1 capture never reads the probes it is baking.
fn entity_probe_sh(
    scene: &Scene,
    id: u32,
    centre: Vec3,
    capture: &crate::render::ibl::cubemap_capture::CaptureMode,
) -> (u32, [[f32; 4]; 9]) {
    let probe_lit =
        || !capture.static_only && scene.world.mesh(id).is_some_and(|m| m.reads_probes());
    if scene.world.is_static(id) && !capture.probe_bounce && !probe_lit() {
        return (0, [[0.0; 4]; 9]);
    }
    probe_sh_at(scene, centre)
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

/// The light probes' shadowmask occlusion at `position` (#1695), all ones when no
/// probe covers it: what a `Mixed` light's static shadow is on a surface no
/// shadowmask page covers (a dynamic object, a static one with no lightmap). A
/// non-finite position reads all ones, never a NaN the shader would spread. A
/// scene whose bake gave no light a channel samples nothing.
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
