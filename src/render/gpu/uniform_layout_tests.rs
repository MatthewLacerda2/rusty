//! Pins the Rust uniform mirrors to `common.wgsl` byte-for-byte (#398, #434): each
//! WGSL struct is composed through the engine's own naga_oil path and each member's
//! offset, plus the total size, is compared against the Rust mirror. A reordered
//! field or a lost pad fails here, not as garbage on the GPU.

use std::mem::{offset_of, size_of};

use super::uniforms::{CameraUniform, LightingUniform};
use crate::render::clusters::LocalLight;
use crate::render::decals::GpuDecal;
use crate::render::passes::shadows::atlas::ShadowTile;
use crate::shadergen::compose;

const PROBE: &str =
    "#import common::{CameraUniforms, Decal, LightingUniforms, LocalLight, ShadowTile}\n\
@group(0) @binding(0) var<uniform> camera: CameraUniforms;\n\
@group(0) @binding(1) var<uniform> lighting: LightingUniforms;\n\
@group(0) @binding(7) var<storage, read> lights: array<LocalLight>;\n\
@group(0) @binding(8) var<storage, read> tiles: array<ShadowTile>;\n\
@group(0) @binding(10) var<storage, read> decals: array<Decal>;\n\
@fragment fn fs_main() -> @location(0) vec4<f32> {\n\
    return vec4<f32>(camera.time + lighting.ssr_active + lights[0].range + tiles[0].params.x\n\
        + decals[0].fade.x);\n\
}\n";

/// `(member offset by name, struct size)` of the WGSL struct named `name`.
fn wgsl_struct(name: &str) -> (Vec<(String, usize)>, usize) {
    let mut composer = compose::composer_with_common("assets/shaders").unwrap();
    let module = compose::compose(&mut composer, PROBE, "probe.wgsl").unwrap();
    let found = module
        .types
        .iter()
        .find_map(|(_, ty)| match &ty.inner {
            wgpu::naga::TypeInner::Struct { members, span }
                if ty.name.as_deref().is_some_and(|n| n.contains(name)) =>
            {
                let members = members
                    .iter()
                    .map(|m| (m.name.clone().unwrap_or_default(), m.offset as usize))
                    .collect();
                Some((members, *span as usize))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{name} in the composed module"));
    found
}

/// Assert each `(wgsl member, rust offset)` pair and the size agree.
fn assert_layout(name: &str, pairs: &[(&str, usize)], size: usize) {
    let (members, span) = wgsl_struct(name);
    for (member, rust) in pairs {
        let wgsl = members
            .iter()
            .find(|(m, _)| m == member)
            .unwrap_or_else(|| panic!("WGSL member `{name}.{member}` missing"))
            .1;
        assert_eq!(wgsl, *rust, "{name}.{member}");
    }
    assert_eq!(span, size, "{name} size");
}

#[test]
fn camera_uniform_matches_wgsl_layout() {
    let pairs = [
        ("view_proj", offset_of!(CameraUniform, view_proj)),
        ("camera_pos", offset_of!(CameraUniform, camera_pos)),
        ("time", offset_of!(CameraUniform, time)),
        ("fog", offset_of!(CameraUniform, fog)),
        ("clusters", offset_of!(CameraUniform, clusters)),
    ];
    assert_layout("CameraUniforms", &pairs, size_of::<CameraUniform>());
    // `time` rides in the vec3 pad (#398); the cluster block adds 48 bytes (#434).
    assert_eq!(size_of::<CameraUniform>(), 176);
}

#[test]
fn lighting_uniform_matches_wgsl_layout() {
    let pairs = [
        ("dir_lights", offset_of!(LightingUniform, dir_lights)),
        (
            "num_dir_lights",
            offset_of!(LightingUniform, num_dir_lights),
        ),
        (
            "ssr_temporal_upsampling",
            offset_of!(LightingUniform, ssr_temporal_upsampling),
        ),
        ("refl_active", offset_of!(LightingUniform, refl_active)),
        ("refl_center", offset_of!(LightingUniform, refl_center)),
        ("refl_box_max", offset_of!(LightingUniform, refl_box_max)),
    ];
    assert_layout("LightingUniforms", &pairs, size_of::<LightingUniform>());
}

#[test]
fn local_light_matches_wgsl_layout() {
    let pairs = [
        ("range", offset_of!(LocalLight, range)),
        ("color", offset_of!(LocalLight, color)),
        ("intensity", offset_of!(LocalLight, intensity)),
        ("direction", offset_of!(LocalLight, direction)),
        ("kind", offset_of!(LocalLight, kind)),
        ("outer_cone", offset_of!(LocalLight, outer_cone)),
        ("shadow", offset_of!(LocalLight, shadow)),
    ];
    assert_layout("LocalLight", &pairs, size_of::<LocalLight>());
}

#[test]
fn shadow_tile_matches_wgsl_layout() {
    let pairs = [
        ("view_proj", offset_of!(ShadowTile, view_proj)),
        ("rect", offset_of!(ShadowTile, rect)),
        ("params", offset_of!(ShadowTile, params)),
    ];
    assert_layout("ShadowTile", &pairs, size_of::<ShadowTile>());
}

#[test]
fn gpu_decal_matches_wgsl_layout() {
    let pairs = [
        ("world_to_decal", offset_of!(GpuDecal, world_to_decal)),
        ("right", offset_of!(GpuDecal, right)),
        ("up", offset_of!(GpuDecal, up)),
        ("forward", offset_of!(GpuDecal, forward)),
        ("color", offset_of!(GpuDecal, color)),
        ("surface", offset_of!(GpuDecal, surface)),
        ("fade", offset_of!(GpuDecal, fade)),
        ("layers", offset_of!(GpuDecal, layers)),
    ];
    assert_layout("Decal", &pairs, size_of::<GpuDecal>());
}
