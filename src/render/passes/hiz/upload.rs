//! Writing one camera's GPU occlusion cull (#1963) into its buffers, and the bind
//! groups `hiz.wgsl` reads them through.

use glam::{Mat4, Vec3};

use super::cull::{CameraCull, ARGS};
use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::render::Renderer;

/// `hiz.wgsl`'s `Params`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Params {
    pub(super) view_proj: [f32; 16],
    size: [f32; 2],
    pub(super) levels: u32,
    instances: u32,
    pub(super) batches: u32,
    _pad: [u32; 3],
}

/// The batches' runs and draws, as `hiz.wgsl`'s `Batch`.
fn pack_batches(batches: &[DrawBatch]) -> Vec<[u32; 4]> {
    let run = |b: &DrawBatch| {
        let (first, count) = (b.instances.start, b.instances.len() as u32);
        [first, count, b.key.indices.len(), b.key.indices.start]
    };
    batches.iter().map(run).collect()
}

/// Each instance's bounds, as `hiz.wgsl`'s `Bounds`: tested (`min.w` 1) when its
/// object is among `tested`, by slot.
pub(super) fn pack_bounds(slots: &[u32], tested: &[(u32, (Vec3, Vec3))]) -> Vec<[f32; 8]> {
    let mut by_slot = vec![None; slots.iter().max().map_or(0, |&s| s as usize + 1)];
    for &(slot, bounds) in tested {
        if let Some(at) = by_slot.get_mut(slot as usize) {
            *at = Some(bounds);
        }
    }
    let one = |&slot: &u32| match by_slot[slot as usize] {
        Some((lo, hi)) => [lo.x, lo.y, lo.z, 1.0, hi.x, hi.y, hi.z, 0.0],
        None => [0.0; 8],
    };
    slots.iter().map(one).collect()
}

impl Renderer {
    /// Write `frame` into `cam`'s buffers and bind them: the phase-one group, then
    /// the test and final group.
    pub(super) fn upload_cull(
        &self,
        cam: &mut CameraCull,
        (frame, pyramid): (&Frame, &wgpu::TextureView),
        n: u32,
    ) -> [wgpu::BindGroup; 2] {
        let (device, queue) = (&self.device, &self.queue);
        let params = Params {
            view_proj: frame.view_proj.to_cols_array(),
            size: [frame.depth.0 as f32, frame.depth.1 as f32],
            levels: frame.levels,
            instances: n,
            batches: frame.batches.len() as u32,
            _pad: [0; 3],
        };
        queue.write_buffer(&cam.params, 0, bytemuck::bytes_of(&params));
        cam.bounds
            .upload(device, queue, bytemuck::cast_slice(frame.bounds));
        let batches = pack_batches(frame.batches);
        cam.batches
            .upload(device, queue, bytemuck::cast_slice(&batches));
        cam.hold(device, queue, frame.slots);
        cam.visible.reserve(device, 4 * u64::from(n));
        let args = ARGS * frame.batches.len() as u64;
        for k in 0..2 {
            cam.kept[k].reserve(device, 16 * u64::from(n));
            cam.args[k].reserve(device, args);
        }
        [0, 1].map(|k| self.cull_group(cam, k, pyramid))
    }

    /// `hiz.wgsl`'s group 0 over `cam`'s buffers, writing phase `k`'s kept
    /// instances and args (0: phase one, 1: the final draws).
    fn cull_group(
        &self,
        cam: &CameraCull,
        k: usize,
        pyramid: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let buffers = [
            &cam.params,
            cam.bounds.buffer(),
            cam.batches.buffer(),
            self.draw_buffers.instances(),
            cam.history.buffer(),
            cam.visible.buffer(),
            cam.kept[k].buffer(),
            cam.args[k].buffer(),
        ];
        let mut entries: Vec<_> = (0..)
            .zip(buffers)
            .map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 8,
            resource: wgpu::BindingResource::TextureView(pyramid),
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hi-Z Cull"),
            layout: &self.hiz.cull_layout,
            entries: &entries,
        })
    }
}

/// What one camera's cull uploads.
pub(super) struct Frame<'a> {
    pub(super) view_proj: Mat4,
    pub(super) depth: (u32, u32),
    pub(super) levels: u32,
    pub(super) batches: &'a [DrawBatch],
    pub(super) bounds: &'a [[f32; 8]],
    pub(super) slots: u32,
}

/// `hiz.wgsl`'s group 0.
pub(super) fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let compute = wgpu::ShaderStages::COMPUTE;
    let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        ..storage_entry(binding, compute)
    };
    let pyramid = wgpu::BindGroupLayoutEntry {
        binding: 8,
        visibility: compute,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Hi-Z Cull"),
        entries: &[
            uniform_entry(0, compute),
            storage(1, true),
            storage(2, true),
            storage(3, true),
            storage(4, false),
            storage(5, false),
            storage(6, false),
            storage(7, false),
            pyramid,
        ],
    })
}
