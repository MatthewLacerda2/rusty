//! src/render/gpu/draw_buffers.rs — the forward pass's per-frame instancing buffers
//! (#470), and the resources every group-1 bind group is built from.
//!
//! This replaces the per-entity pool of #210 (one uniform buffer, bone buffer and two
//! bind groups kept alive per entity). A camera's solids are now three packed arrays —
//! per-draw uniforms at a 256-byte stride, bone palettes, and instances — written with
//! three `write_buffer` calls, behind **one** bind group whose dynamic offsets pick a
//! batch's uniform and palette. Nothing is keyed by entity, so two scenes rendered in
//! one frame (#355) cannot evict each other: each render simply rewrites the arrays
//! before its own submit.

use wgpu::util::DeviceExt;

use super::grow_buffer::GrowBuffer;
use crate::render::{BoneUniform, EntityUniform, InstanceData};

/// Byte distance between per-draw uniforms: wgpu's default
/// `min_uniform_buffer_offset_alignment`, which the renderer requests.
pub(crate) const UNIFORM_STRIDE: usize = 256;
const _: () = assert!(std::mem::size_of::<EntityUniform>() <= UNIFORM_STRIDE);
const _: () = assert!(std::mem::size_of::<BoneUniform>().is_multiple_of(UNIFORM_STRIDE));

/// The identity bone palette — slot 0 of the frame's palettes, and the one overlays bind.
pub(crate) const IDENTITY_BONES: BoneUniform = BoneUniform {
    bones: [glam::Mat4::IDENTITY.to_cols_array(); 64],
};

/// One camera's draw data: `uniforms` in slot order, the skinned `palettes` (bone slot
/// `n` is `palettes[n - 1]`; slot 0 is the identity), and the packed `instances`.
pub(crate) struct FrameUpload<'a> {
    pub uniforms: &'a [EntityUniform],
    pub palettes: &'a [BoneUniform],
    pub instances: &'a [InstanceData],
}

pub(crate) struct DrawBuffers {
    uniforms: GrowBuffer,
    bones: GrowBuffer,
    instances: GrowBuffer,
    bind_group: wgpu::BindGroup,
    /// The identity palette alone, for overlays (they bind their own uniform).
    default_bones: wgpu::Buffer,
    /// A one-element instance array holding [`InstanceData::IDENTITY`], for overlays.
    identity_instance: wgpu::Buffer,
}

impl DrawBuffers {
    pub(crate) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        use wgpu::BufferUsages as U;
        let uniforms = GrowBuffer::new(device, "Frame Draw Uniforms", U::UNIFORM);
        let bones = GrowBuffer::new(device, "Frame Bone Palettes", U::UNIFORM);
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
            bytemuck::bytes_of(&IDENTITY_BONES),
            U::UNIFORM,
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
            palettes,
            instances,
        } = frame;
        let mut uniform_bytes = vec![0u8; uniforms.len() * UNIFORM_STRIDE];
        for (chunk, u) in uniform_bytes.chunks_mut(UNIFORM_STRIDE).zip(uniforms) {
            chunk[..std::mem::size_of::<EntityUniform>()].copy_from_slice(bytemuck::bytes_of(u));
        }
        let mut bone_bytes = bytemuck::bytes_of(&IDENTITY_BONES).to_vec();
        bone_bytes.extend_from_slice(bytemuck::cast_slice(palettes));

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

    /// Dynamic offsets selecting uniform `slot` and bone palette `bones`.
    pub(crate) fn offsets(slot: u32, bones: u32) -> [u32; 2] {
        let palette = std::mem::size_of::<BoneUniform>() as u32;
        [slot * UNIFORM_STRIDE as u32, bones * palette]
    }

    pub(crate) fn default_bones(&self) -> &wgpu::Buffer {
        &self.default_bones
    }

    pub(crate) fn identity_instance(&self) -> &wgpu::Buffer {
        &self.identity_instance
    }
}

/// A group-1 bind group over `[uniform, bones, instances]`. The two uniforms bind
/// one element's width, so a dynamic offset can slide across the packed array.
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
                resource: sized(bones, std::mem::size_of::<BoneUniform>()),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: instances.as_entire_binding(),
            },
        ],
    })
}

/// A skinned mesh's palette: its first ≤64 matrices, the rest identity.
pub(crate) fn palette_uniform(palette: &[glam::Mat4]) -> BoneUniform {
    let mut bones = IDENTITY_BONES;
    for (dst, src) in bones.bones.iter_mut().zip(palette.iter().take(64)) {
        *dst = src.to_cols_array();
    }
    bones
}
