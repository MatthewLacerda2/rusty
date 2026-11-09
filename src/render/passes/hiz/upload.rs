//! Writing one camera's GPU occlusion cull (#1963) into its buffers, and the bind
//! groups `hiz.wgsl` reads them through.

use std::ops::Range;

use glam::{Mat4, Vec3};

use super::cull::{CameraCull, ARGS};
use super::kept::Tables;
use crate::render::draw::batch::{DrawBatch, OpaqueBatches};
use crate::render::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::render::{InstanceData, Renderer};

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
/// `clustered` says, per batch, whether its final draws are by cluster (#1968).
fn pack_batches(batches: &OpaqueBatches, clustered: &[Option<Range<u32>>]) -> Vec<[u32; 8]> {
    let run = |(b, by_cluster): (&DrawBatch, &Option<Range<u32>>)| {
        let (first, count) = (b.instances.start, b.instances.len() as u32);
        let flag = u32::from(by_cluster.is_some());
        [
            first,
            count,
            b.key.indices.len(),
            b.key.indices.start,
            flag,
            0,
            0,
            0,
        ]
    };
    batches.iter().zip(clustered).map(run).collect()
}

/// Fill `tables.bounds` with each of `instances`' bounds, as `hiz.wgsl`'s
/// `Bounds`: tested (`min.w` 1) when its object is among `tested`, by slot, and
/// `tables.slots` with their object slots. Returns how many are tested. Reuses
/// the tables' room, so a frame allocates nothing once they fit the scene.
pub(super) fn pack_bounds(
    tables: &mut Tables,
    instances: &[InstanceData],
    tested: &[(u32, (Vec3, Vec3))],
) -> u32 {
    let Tables {
        slots,
        by_slot,
        bounds,
    } = tables;
    slots.clear();
    slots.extend(instances.iter().map(|i| i.object));
    by_slot.clear();
    by_slot.resize(slots.iter().max().map_or(0, |&s| s as usize + 1), None);
    for &(slot, b) in tested {
        if let Some(at) = by_slot.get_mut(slot as usize) {
            *at = Some(b);
        }
    }
    let one = |&slot: &u32| match by_slot[slot as usize] {
        Some((lo, hi)) => [lo.x, lo.y, lo.z, 1.0, hi.x, hi.y, hi.z, 0.0],
        None => [0.0; 8],
    };
    bounds.clear();
    bounds.extend(slots.iter().map(one));
    bounds.iter().filter(|b| b[3] > 0.5).count() as u32
}

impl Renderer {
    /// Write `frame` and `cam`'s packed bounds into its buffers, and bind them
    /// (`cam.groups`: the phase-one group, then the test and final group), rebuilt
    /// only when one of those buffers or the pyramid was replaced.
    pub(super) fn upload_cull(
        &self,
        cam: &mut CameraCull,
        (frame, pyramid): (&Frame, &wgpu::TextureView),
        n: u32,
    ) {
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
        let bounds = bytemuck::cast_slice(&cam.tables.bounds);
        cam.bounds.upload(device, queue, bounds);
        let batches = pack_batches(frame.batches, frame.clustered);
        cam.batches
            .upload(device, queue, bytemuck::cast_slice(&batches));
        cam.hold(device, queue, frame.slots);
        cam.visible.reserve(device, 4 * u64::from(n));
        let args = ARGS * frame.batches.len() as u64;
        for k in 0..2 {
            cam.kept[k].reserve(device, 16 * u64::from(n));
            cam.args[k].reserve(device, args);
        }
        let key = [0, 1].map(|k| self.cull_buffers(cam, k).map(Clone::clone));
        let mut groups = std::mem::take(&mut cam.groups);
        groups.get((key, pyramid.clone()), || {
            [0, 1].map(|k| self.cull_group(cam, k, pyramid))
        });
        cam.groups = groups;
    }

    /// The buffers `hiz.wgsl`'s group 0 binds for phase `k`, by binding.
    fn cull_buffers<'a>(&'a self, cam: &'a CameraCull, k: usize) -> [&'a wgpu::Buffer; 8] {
        [
            &cam.params,
            cam.bounds.buffer(),
            cam.batches.buffer(),
            self.draw_buffers.instances(),
            cam.history.buffer(),
            cam.visible.buffer(),
            cam.kept[k].buffer(),
            cam.args[k].buffer(),
        ]
    }

    /// `hiz.wgsl`'s group 0 over `cam`'s buffers, writing phase `k`'s kept
    /// instances and args (0: phase one, 1: the final draws).
    fn cull_group(
        &self,
        cam: &CameraCull,
        k: usize,
        pyramid: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let buffers = self.cull_buffers(cam, k);
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
    pub(super) batches: &'a OpaqueBatches,
    /// Per batch, the cluster draws standing for it (#1968).
    pub(super) clustered: &'a [Option<Range<u32>>],
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
