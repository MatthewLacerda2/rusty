//! src/render/gpu/draw_buffers/pack.rs — the byte layouts the packed per-frame arrays
//! take (#470): per-draw uniforms at their dynamic-offset stride, and the joint array
//! with its shared identity at element 0, and the group-1 bind group over them.

use super::{IDENTITY_JOINT, UNIFORM_STRIDE};
use crate::render::DrawUniform;

/// One joint matrix, as the storage array packs it.
pub(crate) type JointMatrix = [f32; 16];

/// `uniforms` at [`UNIFORM_STRIDE`], the layout a dynamic offset of
/// [`DrawBuffers::offsets`] slides across; editor overlays (#1616) and the shadow
/// casters' draws pack theirs through this too.
pub(crate) fn uniform_array_bytes(uniforms: &[DrawUniform]) -> Vec<u8> {
    let mut bytes = vec![0u8; uniforms.len() * UNIFORM_STRIDE];
    for (chunk, u) in bytes.chunks_mut(UNIFORM_STRIDE).zip(uniforms) {
        chunk[..std::mem::size_of::<DrawUniform>()].copy_from_slice(bytemuck::bytes_of(u));
    }
    bytes
}

/// The joint storage array as uploaded: the shared identity at element 0, then
/// `joints` — the layout every `bone_base` from [`push_palette`] indexes. The shadow
/// pass (#599) uploads its casters' palettes through this too.
pub(crate) fn joint_array_bytes(joints: &[JointMatrix]) -> Vec<u8> {
    let mut bytes = bytemuck::bytes_of(&IDENTITY_JOINT).to_vec();
    bytes.extend_from_slice(bytemuck::cast_slice(joints));
    bytes
}

/// Append a draw's joint `palette` to the frame's `joints` and return its `bone_base`
/// — every joint kept, however many (#455). An empty palette (a non-skinned mesh)
/// appends nothing and returns `0`, the shared identity.
pub(crate) fn push_palette(joints: &mut Vec<JointMatrix>, palette: &[glam::Mat4]) -> u32 {
    if palette.is_empty() {
        return 0;
    }
    // Element 0 of the uploaded array is the identity, so this run starts one later.
    let base = joints.len() as u32 + 1;
    joints.extend(palette.iter().map(glam::Mat4::to_cols_array));
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    #[test]
    fn every_joint_of_a_large_skin_is_kept_past_64() {
        let rig: Vec<Mat4> = (0..100)
            .map(|j| Mat4::from_translation(Vec3::X * j as f32))
            .collect();
        let mut joints = Vec::new();
        assert_eq!(push_palette(&mut joints, &[]), 0, "non-skinned: identity");
        assert!(joints.is_empty(), "non-skinned draws allocate no palette");
        assert_eq!(push_palette(&mut joints, &rig), 1);
        assert_eq!(
            push_palette(&mut joints, &rig[..3]),
            101,
            "packed back to back"
        );
        assert_eq!(joints.len(), 103);
        assert_eq!(joints[99][12], 99.0, "joint 99 is posed, not identity");
    }
}

/// A group-1 bind group over `[uniform, bones, instances, materials, proxies,
/// objects]`. The uniform binds one element's width, so a dynamic offset can slide
/// across the packed array; the other arrays bind whole, and the shader indexes them.
pub(crate) fn group1(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    [uniform, bones, instances, materials, proxies, objects]: [&wgpu::Buffer; 6],
) -> wgpu::BindGroup {
    let sized = |buffer, size: usize| {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(size as u64),
        })
    };
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: sized(uniform, std::mem::size_of::<DrawUniform>()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bones.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: instances.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: materials.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: proxies.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: objects.as_entire_binding(),
            },
        ],
    })
}
