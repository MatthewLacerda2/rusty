//! A sweep's caster data on the GPU: the per-caster array `shadow.wgsl` indexes by
//! instance, and the joint array its skinned casters' palettes live in (#599).

use crate::render::gpu::draw_buffers::{joint_array_bytes, JointMatrix};
use crate::render::gpu::grow_buffer::GrowBuffer;

/// One caster as `shadow.wgsl`'s `Caster` reads it: world matrix, then where its
/// palette starts in the joint array (0 — the identity — for an unskinned mesh).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::render::passes::shadows) struct CasterData {
    pub model: [f32; 16],
    pub bone_base: u32,
    _pad: [u32; 3],
}

impl CasterData {
    pub fn new(model: glam::Mat4, bone_base: u32) -> Self {
        Self {
            model: model.to_cols_array(),
            bone_base,
            _pad: [0; 3],
        }
    }
}

/// The caster and joint arrays and the bind group that exposes them to `shadow.wgsl`.
pub(in crate::render::passes::shadows) struct CasterBuffer {
    casters: GrowBuffer,
    joints: GrowBuffer,
    pub(super) bind_group: wgpu::BindGroup,
}

impl CasterBuffer {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let storage = wgpu::BufferUsages::STORAGE;
        let casters = GrowBuffer::new(device, "Shadow Casters", storage);
        let joints = GrowBuffer::new(device, "Shadow Caster Joints", storage);
        let bind_group = caster_group(device, layout, &casters, &joints);
        Self {
            casters,
            joints,
            bind_group,
        }
    }

    /// Upload the sweep's `casters` and `joints`, rebuilding the bind group if either
    /// buffer had to grow.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        casters: &[CasterData],
        joints: &[JointMatrix],
    ) {
        let grew = [
            self.casters
                .upload(device, queue, bytemuck::cast_slice(casters)),
            self.joints
                .upload(device, queue, &joint_array_bytes(joints)),
        ];
        if grew.contains(&true) {
            self.bind_group = caster_group(device, layout, &self.casters, &self.joints);
        }
    }
}

fn caster_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    casters: &GrowBuffer,
    joints: &GrowBuffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Caster Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: casters.buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: joints.buffer().as_entire_binding(),
            },
        ],
    })
}
