//! The cluster storage buffers (#434), bound in group 0 at bindings 7–9: the frame's
//! lights, each cluster's `[offset, count]`, and the flat light-index list.

use std::time::Instant;

use super::{bin, ClusterGrid, LocalLight};
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::Renderer;

pub(crate) struct ClusterBuffers {
    lights: GrowBuffer,
    ranges: GrowBuffer,
    indices: GrowBuffer,
    /// The frame's lights as uploaded: what every camera of the frame bins.
    frame_lights: Vec<LocalLight>,
}

impl ClusterBuffers {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let storage = |label| GrowBuffer::new(device, label, wgpu::BufferUsages::STORAGE);
        Self {
            lights: storage("Cluster Lights"),
            ranges: storage("Cluster Ranges"),
            indices: storage("Cluster Light Indices"),
            frame_lights: Vec::new(),
        }
    }

    /// The buffers at group-0 bindings 7, 8 and 9.
    pub(crate) fn buffers(&self) -> [&wgpu::Buffer; 3] {
        [
            self.lights.buffer(),
            self.ranges.buffer(),
            self.indices.buffer(),
        ]
    }
}

impl Renderer {
    /// Upload the frame's point and spot lights, once per frame.
    pub(crate) fn upload_local_lights(&mut self, lights: Vec<LocalLight>) {
        let c = &mut self.clusters;
        let bytes = bytemuck::cast_slice(&lights);
        self.global_bind_group_dirty |= c.lights.upload(&self.device, &self.queue, bytes);
        c.frame_lights = lights;
    }

    /// Bin the frame's lights into one camera's clusters and upload its lists,
    /// counting what it saw, culled and dropped, and how long it took.
    pub(crate) fn bin_lights(&mut self, grid: &ClusterGrid) {
        let started = Instant::now();
        let c = &mut self.clusters;
        let binned = bin(grid, &c.frame_lights);
        let (device, queue) = (&self.device, &self.queue);
        let mut grown = c
            .ranges
            .upload(device, queue, bytemuck::cast_slice(&binned.ranges));
        grown |= c
            .indices
            .upload(device, queue, bytemuck::cast_slice(&binned.indices));
        self.global_bind_group_dirty |= grown;
        let counters = &mut self.frame_counters;
        counters.lights_visible += binned.visible;
        counters.lights_culled += binned.culled;
        counters.lights_dropped += binned.dropped;
        counters.light_cluster_refs += binned.indices.len() as u64;
        counters.light_bin_us += started.elapsed().as_micros() as u64;
    }
}
