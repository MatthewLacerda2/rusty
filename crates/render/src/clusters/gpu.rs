use std::time::Instant;

use super::bin::{bin_spheres, Binned, Budget};
use super::grid::AabbCache;
use super::{bin, ClusterGrid, LocalLight, CLUSTER_COUNT};
use crate::gpu::grow_buffer::GrowBuffer;
use crate::{RenderSwitch, Renderer};

pub(crate) struct ClusterBuffers {
    lights: GrowBuffer,
    ranges: GrowBuffer,
    indices: GrowBuffer,
    frame_lights: Vec<LocalLight>,
    shadow_requests: Vec<bool>,
    aabbs: AabbCache,
}

impl ClusterBuffers {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let storage = |label| GrowBuffer::new(device, label, wgpu::BufferUsages::STORAGE);
        Self {
            lights: storage("Cluster Lights"),
            ranges: storage("Cluster Ranges"),
            indices: storage("Cluster Light Indices"),
            frame_lights: Vec::new(),
            shadow_requests: Vec::new(),
            aabbs: AabbCache::default(),
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

impl ClusterBuffers {
    pub(crate) fn stage(&mut self, lights: Vec<(LocalLight, bool)>) {
        (self.frame_lights, self.shadow_requests) = lights.into_iter().unzip();
    }

    pub(crate) fn staged(&self) -> Vec<(LocalLight, bool)> {
        let requests = self.shadow_requests.iter().copied();
        self.frame_lights.iter().copied().zip(requests).collect()
    }

    pub(crate) fn set_shadow(&mut self, light: usize, first_tile: u32) {
        self.frame_lights[light].shadow = first_tile;
    }
}

impl Renderer {
    /// Upload the frame's staged point and spot lights, once per frame.
    pub(crate) fn upload_local_lights(&mut self) {
        let c = &mut self.clusters;
        let bytes = bytemuck::cast_slice(&c.frame_lights);
        self.global_bind_group_dirty |= c.lights.upload(&self.device, &self.queue, bytes);
    }

    pub(crate) fn bin_clusters(&mut self, grid: &ClusterGrid, decals: bool) {
        let started = Instant::now();
        let c = &mut self.clusters;
        // With the `clustered_lights` switch off none is binned: every range empty.
        let frame = (self.switches).listed(RenderSwitch::ClusteredLights, &c.frame_lights);
        let lights = bin(grid, frame, &mut c.aabbs);
        let spheres: &[_] = if decals { &self.decals.spheres } else { &[] };
        let stamps = bin_spheres(grid, spheres, &Budget::nearest(spheres.len()), &mut c.aabbs);
        let (ranges, indices) = concat(&lights, &stamps);
        let (device, queue) = (&self.device, &self.queue);
        let mut grown = c
            .ranges
            .upload(device, queue, bytemuck::cast_slice(&ranges));
        grown |= c
            .indices
            .upload(device, queue, bytemuck::cast_slice(&indices));
        self.global_bind_group_dirty |= grown;
        let counters = &mut self.frame_counters;
        counters.lights_visible += lights.visible;
        counters.lights_culled += lights.culled;
        counters.lights_dropped += lights.dropped;
        counters.cluster_lights_dropped += lights.cluster_dropped;
        counters.light_cluster_refs += lights.indices.len() as u64;
        counters.decals_visible += stamps.visible;
        counters.decal_cluster_refs += stamps.indices.len() as u64;
        counters.light_bin_us += started.elapsed().as_micros() as u64;
    }
}

pub(super) fn concat(lights: &Binned, decals: &Binned) -> (Vec<[u32; 2]>, Vec<u32>) {
    let base = lights.indices.len() as u32;
    let mut ranges = Vec::with_capacity(2 * CLUSTER_COUNT);
    ranges.extend_from_slice(&lights.ranges);
    ranges.extend(
        decals
            .ranges
            .iter()
            .map(|&[offset, count]| [offset + base, count]),
    );
    let indices = [lights.indices.as_slice(), &decals.indices].concat();
    (ranges, indices)
}
