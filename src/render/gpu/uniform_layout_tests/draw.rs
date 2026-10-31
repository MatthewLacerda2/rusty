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

/// `(member name, its byte offset)` for each named field of `$ty`, `$base` bytes in:
/// the WGSL side writes nested structs out flat, under the same names.
macro_rules! members {
    ($ty:ty, $base:expr; $($field:ident),* $(,)?) => {
        [$((stringify!($field), $base + offset_of!($ty, $field))),*]
    };
}

#[test]
fn draw_uniform_matches_wgsl_layout() {
    let pairs = members!(DrawUniform, 0; model_matrix, bone_base, material);
    let found = wgsl_struct_in(PROBE, "DrawUniforms");
    assert_members(found, "DrawUniforms", &pairs, size_of::<DrawUniform>());
}

#[test]
fn material_data_matches_wgsl_layout() {
    let surface = offset_of!(MaterialData, surface);
    let mapping = offset_of!(MaterialData, mapping);
    let own = members!(MaterialData, 0;
        color_tint, use_texture, is_lit, metallic, roughness, use_metallic_map,
        use_roughness_map, use_normal_map, use_emissive_map, emissive, use_cutout,
        alpha_cutoff, receive_decals, two_sided,
    );
    let surface = members!(SurfaceUniform, surface;
        blend_mode, premultiply_diffuse, receive_shadows, refraction_model, ior,
        thickness, uses_opaque, depth_postpass, absorption, transmission, receive_ssr,
    );
    let mapping = members!(MappingUniform, mapping;
        uv_st, uv_mapping, world_scale, blend_sharpness,
    );
    let pairs: Vec<_> = [&own[..], &surface, &mapping].concat();
    let found = wgsl_struct_in(PROBE, "MaterialData");
    assert_members(found, "MaterialData", &pairs, size_of::<MaterialData>());
}
