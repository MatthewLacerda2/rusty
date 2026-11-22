use super::{IDENTITY_JOINT, UNIFORM_STRIDE};
use crate::gpu::ops::CountedDevice as _;
use crate::DrawUniform;

pub(crate) type JointMatrix = [f32; 16];

pub(crate) fn uniform_array_bytes(uniforms: &[DrawUniform]) -> Vec<u8> {
    let mut bytes = vec![0u8; uniforms.len() * UNIFORM_STRIDE];
    for (chunk, u) in bytes.chunks_mut(UNIFORM_STRIDE).zip(uniforms) {
        chunk[..std::mem::size_of::<DrawUniform>()].copy_from_slice(bytemuck::bytes_of(u));
    }
    bytes
}

pub(crate) fn joint_array_bytes(joints: &[JointMatrix]) -> Vec<u8> {
    let mut bytes = bytemuck::bytes_of(&IDENTITY_JOINT).to_vec();
    bytes.extend_from_slice(bytemuck::cast_slice(joints));
    bytes
}

pub(crate) fn push_palette(joints: &mut Vec<JointMatrix>, palette: &[glam::Mat4]) -> u32 {
    if palette.is_empty() {
        return 0;
    }
    let base = joints.len() as u32 + 1;
    joints.extend(palette.iter().map(glam::Mat4::to_cols_array));
    base
}

pub(crate) fn draw_group(
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
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
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
