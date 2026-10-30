//! The forward pass's per-draw uniform (`forward/bindings.wgsl`'s `EntityUniforms`)
//! against `EntityUniform`, its transparent-surface tail included (#1523).

use std::mem::{offset_of, size_of};

use super::{assert_members, wgsl_struct_in};
use crate::render::gpu::uniforms::{MappingUniform, SurfaceUniform};
use crate::render::EntityUniform;

const PROBE: &str = "#import forward::bindings::entity\n\
@fragment fn fs_main() -> @location(0) vec4<f32> {\n\
    return entity.color_tint + entity.transmission;\n\
}\n";

#[test]
fn entity_uniform_matches_wgsl_layout() {
    let surface = offset_of!(EntityUniform, surface);
    let at = |field: usize| surface + field;
    let mapping = offset_of!(EntityUniform, mapping);
    let map = |field: usize| mapping + field;
    let pairs = [
        ("color_tint", offset_of!(EntityUniform, color_tint)),
        ("emissive", offset_of!(EntityUniform, emissive)),
        ("receive_decals", offset_of!(EntityUniform, receive_decals)),
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
    let found = wgsl_struct_in(PROBE, "EntityUniforms");
    assert_members(found, "EntityUniforms", &pairs, size_of::<EntityUniform>());
}
