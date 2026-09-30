//! src/render/gpu/draw_buffers.rs — the forward pass's per-frame instancing buffers
//! (#470), and the resources every group-1 bind group is built from.
//!
//! This replaces the per-entity pool of #210 (one uniform buffer, bone buffer and two
//! bind groups kept alive per entity). A camera's solids are now three packed arrays —
//! per-draw uniforms at a 256-byte stride, joint matrices, and instances — written with
//! three `write_buffer` calls, behind **one** bind group whose dynamic offset picks a
//! batch's uniform. Nothing is keyed by entity, so two scenes rendered in one frame
//! (#355) cannot evict each other: each render simply rewrites the arrays before its
//! own submit.
//!
//! The joint matrices are one storage array (#455), not a fixed 64-matrix uniform: a
//! skinned draw's palette is its run of the array, as long as its skin, and the draw's
//! uniform carries where that run starts (`bone_base`). Element 0 is one identity
//! matrix, which every non-skinned draw and overlay points at.

use wgpu::util::DeviceExt;

use super::grow_buffer::GrowBuffer;
use crate::render::{EntityUniform, InstanceData};

/// Byte distance between per-draw uniforms: wgpu's default
/// `min_uniform_buffer_offset_alignment`, which the renderer requests.
pub(crate) const UNIFORM_STRIDE: usize = 256;
const _: () = assert!(std::mem::size_of::<EntityUniform>() <= UNIFORM_STRIDE);

/// The shared identity joint: element 0 of the frame's joint array, and the whole of
/// the array overlays bind.
const IDENTITY_JOINT: [f32; 16] = glam::Mat4::IDENTITY.to_cols_array();

/// One joint matrix, as the storage array packs it.
pub(crate) type JointMatrix = [f32; 16];

/// One camera's draw data: `uniforms` in slot order, the skinned draws' `joints`
/// back to back (a draw whose `bone_base` is `n` starts at `joints[n - 1]`; element 0
/// of the uploaded array is the identity), and the packed `instances`.
pub(crate) struct FrameUpload<'a> {
    pub uniforms: &'a [EntityUniform],
    pub joints: &'a [JointMatrix],
    pub instances: &'a [InstanceData],
}

pub(crate) struct DrawBuffers {
    uniforms: GrowBuffer,
    bones: GrowBuffer,
    instances: GrowBuffer,
    bind_group: wgpu::BindGroup,
    /// The identity joint alone, for overlays (they bind their own uniform).
    default_bones: wgpu::Buffer,
    /// A one-element instance array holding [`InstanceData::IDENTITY`], for overlays.
    identity_instance: wgpu::Buffer,
}

impl DrawBuffers {
    pub(crate) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        use wgpu::BufferUsages as U;
        let uniforms = GrowBuffer::new(device, "Frame Draw Uniforms", U::UNIFORM);
        let bones = GrowBuffer::new(device, "Frame Joint Matrices", U::STORAGE);
        let instances = GrowBuffer::new(device, "Frame Instances", U::STORAGE);
        let init = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let default_bones = init(
            "Shared Default Bones",
            bytemuck::bytes_of(&IDENTITY_JOINT),
            U::STORAGE,
        );
        let identity_instance = init(
            "Identity Instance",
            bytemuck::bytes_of(&InstanceData::IDENTITY),
            U::STORAGE,
        );
        let bind_group = group1(
            device,
            layout,
            "Frame Draw Bind Group",
            [uniforms.buffer(), bones.buffer(), instances.buffer()],
        );
        Self {
            uniforms,
            bones,
            instances,
            bind_group,
            default_bones,
            identity_instance,
        }
    }

    /// Write one camera's draw data. Rebuilds the bind group only when a buffer had
    /// to grow.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        frame: FrameUpload,
    ) {
        let FrameUpload {
            uniforms,
            joints,
            instances,
        } = frame;
        let mut uniform_bytes = vec![0u8; uniforms.len() * UNIFORM_STRIDE];
        for (chunk, u) in uniform_bytes.chunks_mut(UNIFORM_STRIDE).zip(uniforms) {
            chunk[..std::mem::size_of::<EntityUniform>()].copy_from_slice(bytemuck::bytes_of(u));
        }
        let bone_bytes = joint_array_bytes(joints);

        let grew = [
            self.uniforms.upload(device, queue, &uniform_bytes),
            self.bones.upload(device, queue, &bone_bytes),
            self.instances
                .upload(device, queue, bytemuck::cast_slice(instances)),
        ];
        if grew.contains(&true) {
            self.bind_group = group1(
                device,
                layout,
                "Frame Draw Bind Group",
                [
                    self.uniforms.buffer(),
                    self.bones.buffer(),
                    self.instances.buffer(),
                ],
            );
        }
    }

    /// The one group-1 bind group every solid draw binds, at [`Self::offsets`].
    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// The dynamic offset selecting uniform `slot`.
    pub(crate) fn offsets(slot: u32) -> [u32; 1] {
        [slot * UNIFORM_STRIDE as u32]
    }

    pub(crate) fn default_bones(&self) -> &wgpu::Buffer {
        &self.default_bones
    }

    pub(crate) fn identity_instance(&self) -> &wgpu::Buffer {
        &self.identity_instance
    }
}

/// A group-1 bind group over `[uniform, bones, instances]`. The uniform binds one
/// element's width, so a dynamic offset can slide across the packed array; the joint
/// and instance arrays bind whole, and the shader indexes them.
pub(crate) fn group1(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    [uniform, bones, instances]: [&wgpu::Buffer; 3],
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
                resource: sized(uniform, std::mem::size_of::<EntityUniform>()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bones.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: instances.as_entire_binding(),
            },
        ],
    })
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
