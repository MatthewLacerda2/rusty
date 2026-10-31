//! The forward pass's per-draw uniform and material record (#1616):
//! `forward/bindings.wgsl`'s `DrawUniforms` and `MaterialData` against `DrawUniform`
//! and `MaterialData`, member by member, the record's flat surface (#1523) and
//! mapping (#1573) tails included.

use std::mem::{offset_of, size_of};

use super::{assert_members, wgsl_struct_in};
use crate::render::gpu::uniforms::{MappingUniform, SurfaceUniform};
use crate::render::{DrawUniform, MaterialData};

const PROBE: &str = "#import forward::bindings::{draw, material}\n\
@fragment fn fs_main() -> @location(0) vec4<f32> {\n\
    return material().color_tint + draw.model_matrix[0];\n\
}\n";

#[test]
fn draw_uniform_matches_wgsl_layout() {
    let pairs = [
        ("model_matrix", offset_of!(DrawUniform, model_matrix)),
        ("bone_base", offset_of!(DrawUniform, bone_base)),
        ("material", offset_of!(DrawUniform, material)),
    ];
    let found = wgsl_struct_in(PROBE, "DrawUniforms");
    assert_members(found, "DrawUniforms", &pairs, size_of::<DrawUniform>());
}

#[test]
fn material_data_matches_wgsl_layout() {
    let surface = offset_of!(MaterialData, surface);
    let at = |field: usize| surface + field;
    let mapping = offset_of!(MaterialData, mapping);
    let map = |field: usize| mapping + field;
    let pairs = [
        ("color_tint", offset_of!(MaterialData, color_tint)),
        ("use_texture", offset_of!(MaterialData, use_texture)),
        ("is_lit", offset_of!(MaterialData, is_lit)),
        ("metallic", offset_of!(MaterialData, metallic)),
        ("roughness", offset_of!(MaterialData, roughness)),
        (
            "use_metallic_map",
            offset_of!(MaterialData, use_metallic_map),
        ),
        (
            "use_roughness_map",
            offset_of!(MaterialData, use_roughness_map),
        ),
        ("use_normal_map", offset_of!(MaterialData, use_normal_map)),
        (
            "use_emissive_map",
            offset_of!(MaterialData, use_emissive_map),
        ),
        ("emissive", offset_of!(MaterialData, emissive)),
        ("use_cutout", offset_of!(MaterialData, use_cutout)),
        ("alpha_cutoff", offset_of!(MaterialData, alpha_cutoff)),
        ("receive_decals", offset_of!(MaterialData, receive_decals)),
        ("blend_mode", at(offset_of!(SurfaceUniform, blend_mode))),
        (
            "premultiply_diffuse",
            at(offset_of!(SurfaceUniform, premultiply_diffuse)),
        ),
        (
            "receive_shadows",
            at(offset_of!(SurfaceUniform, receive_shadows)),
        ),
        (
            "refraction_model",
            at(offset_of!(SurfaceUniform, refraction_model)),
        ),
        ("ior", at(offset_of!(SurfaceUniform, ior))),
        ("thickness", at(offset_of!(SurfaceUniform, thickness))),
        ("uses_opaque", at(offset_of!(SurfaceUniform, uses_opaque))),
        (
            "depth_postpass",
            at(offset_of!(SurfaceUniform, depth_postpass)),
        ),
        ("absorption", at(offset_of!(SurfaceUniform, absorption))),
        ("transmission", at(offset_of!(SurfaceUniform, transmission))),
        ("receive_ssr", at(offset_of!(SurfaceUniform, receive_ssr))),
        ("uv_st", map(offset_of!(MappingUniform, uv_st))),
        ("uv_mapping", map(offset_of!(MappingUniform, uv_mapping))),
        ("world_scale", map(offset_of!(MappingUniform, world_scale))),
        (
            "blend_sharpness",
            map(offset_of!(MappingUniform, blend_sharpness)),
        ),
    ];
    let found = wgsl_struct_in(PROBE, "MaterialData");
    assert_members(found, "MaterialData", &pairs, size_of::<MaterialData>());
}
