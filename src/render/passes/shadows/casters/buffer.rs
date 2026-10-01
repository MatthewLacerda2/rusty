//! A sweep's caster data on the GPU: the per-caster array `shader.wgsl`'s shadow stage
//! indexes by instance, and the joint array its skinned casters' palettes live in (#599).

use crate::render::gpu::draw_buffers::{joint_array_bytes, JointMatrix};
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::passes::shadows::setup::{CASTERS_BINDING, JOINTS_BINDING};

/// One caster as `shader.wgsl`'s `Caster` reads it: world matrix, where its palette
/// starts in the joint array (0 — the identity — for an unskinned mesh), and its
/// cutout alpha test (#648; a cutoff of 0 clips nothing).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::render::passes::shadows) struct CasterData {
    pub model: [f32; 16],
    pub bone_base: u32,
    pub alpha_cutoff: f32,
    pub use_texture: u32,
    _pad: u32,
}

impl CasterData {
    /// A caster whose shadow is not alpha-tested; [`Self::cutout`] adds the test.
    pub fn new(model: glam::Mat4, bone_base: u32) -> Self {
        Self {
            model: model.to_cols_array(),
            bone_base,
            alpha_cutoff: 0.0,
            use_texture: 0,
            _pad: 0,
        }
    }

    /// This caster, its shadow clipped where its albedo's alpha (when `textured`) is
    /// below `cutoff` — a Cutout material's test (#242, #648).
    pub fn cutout(self, cutoff: f32, textured: bool) -> Self {
        Self {
            alpha_cutoff: cutoff,
            use_texture: u32::from(textured),
            ..self
        }
    }
}

/// The caster and joint arrays and the bind group that exposes them to the shadow stage.
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
                binding: CASTERS_BINDING,
                resource: casters.buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: JOINTS_BINDING,
                resource: joints.buffer().as_entire_binding(),
            },
        ],
    })
}
