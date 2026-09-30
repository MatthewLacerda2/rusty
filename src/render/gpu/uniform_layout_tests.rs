//! Pins `CameraUniform` to `common.wgsl`'s `CameraUniforms` byte-for-byte (#398):
//! the WGSL struct is composed through the engine's own naga_oil path and each
//! member's offset, plus the total size, is compared against the Rust mirror. A
//! reordered field or a lost pad fails here, not as garbage on the GPU.

use std::mem::{offset_of, size_of};

use super::shaders::ShaderRegistry;
use super::uniforms::CameraUniform;

const PROBE: &str = "#import common::{CameraUniforms}\n\
@group(0) @binding(0) var<uniform> camera: CameraUniforms;\n\
@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(camera.time); }\n";

#[test]
fn camera_uniform_matches_wgsl_layout() {
    let mut composer = ShaderRegistry::composer_with_common("assets/shaders").unwrap();
    let module = ShaderRegistry::validate_source(&mut composer, PROBE).unwrap();
    let (members, span) = module
        .types
        .iter()
        .find_map(|(_, ty)| match &ty.inner {
            wgpu::naga::TypeInner::Struct { members, span }
                if ty
                    .name
                    .as_deref()
                    .is_some_and(|n| n.contains("CameraUniforms")) =>
            {
                Some((members.clone(), *span))
            }
            _ => None,
        })
        .expect("CameraUniforms in the composed module");
    let offset = |name: &str| {
        members
            .iter()
            .find(|m| m.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("WGSL member `{name}` missing"))
            .offset as usize
    };
    assert_eq!(offset("view_proj"), offset_of!(CameraUniform, view_proj));
    assert_eq!(offset("camera_pos"), offset_of!(CameraUniform, camera_pos));
    assert_eq!(offset("time"), offset_of!(CameraUniform, time));
    assert_eq!(offset("fog"), offset_of!(CameraUniform, fog));
    assert_eq!(span as usize, size_of::<CameraUniform>());
    // `time` rides in the vec3 pad: the struct did not grow (#398).
    assert_eq!(size_of::<CameraUniform>(), 128);
}
