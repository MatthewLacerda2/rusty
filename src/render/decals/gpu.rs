//! The frame's decals on the GPU (#638): one record array uploaded once per frame,
//! and the atlas their maps live in, bound in group 0 at bindings 10–13. Each
//! camera bins the same records into its clusters with its lights
//! (`Renderer::bin_clusters`).

use std::collections::BTreeMap;

use glam::Vec3;

use super::atlas::DecalAtlas;
use super::record::{self, DecalMaps, GpuDecal, NO_LAYER};
use crate::components::MaterialAsset;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::Renderer;
use crate::scene::decal::{Decal, DecalPose};
use crate::scene::Scene;

pub(crate) struct DecalBuffers {
    records: GrowBuffer,
    pub(crate) atlas: DecalAtlas,
    /// The frame's decals' bounding spheres, in record order, for binning.
    pub(crate) spheres: Vec<(Vec3, f32)>,
}

impl DecalBuffers {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            records: GrowBuffer::new(device, "Decal Records", wgpu::BufferUsages::STORAGE),
            atlas: DecalAtlas::new(device),
            spheres: Vec::new(),
        }
    }

    /// The record buffer group 0 binds at binding 10.
    pub(crate) fn records(&self) -> &wgpu::Buffer {
        self.records.buffer()
    }
}

impl Renderer {
    /// Upload the scene's decals, once per frame: load any new maps into the atlas,
    /// then write every record, oldest first (the FIFO order they blend in). Each
    /// decal draws at its pose this frame (#639): an owned one follows its owner
    /// through the world-matrix store, which must already be refreshed, and one
    /// whose owner is inactive or that has faded out is left out.
    pub(crate) fn upload_decals(&mut self, scene: &Scene) {
        let material = |d: &Decal| resolve(&scene.materials, d);
        let drawn: Vec<(&Decal, DecalPose, DecalMaps)> = scene
            .decals
            .iter()
            .filter_map(|d| Some((d, scene.decal_pose(d)?, record::maps(d, material(d)))))
            .collect();
        let paths: Vec<&str> = drawn
            .iter()
            .flat_map(|(_, _, maps)| maps.iter().flatten())
            .map(String::as_str)
            .collect();
        let misses = &mut self.texture_freshness.misses;
        let d = &mut self.decals;
        self.global_bind_group_dirty |= d.atlas.prepare(&self.device, &self.queue, &paths, misses);

        let mut dropped = 0;
        let records: Vec<GpuDecal> = drawn
            .iter()
            .map(|(decal, pose, maps)| {
                let layers = maps.clone().map(|path| {
                    let Some(path) = path else {
                        return NO_LAYER;
                    };
                    let layer = d.atlas.layer(&path);
                    dropped += u32::from(layer == NO_LAYER && !misses.contains(&path));
                    layer
                });
                record::record(decal, pose, material(decal), layers)
            })
            .collect();
        d.spheres = drawn
            .iter()
            .map(|(_, pose, _)| record::bounds(pose))
            .collect();
        let bytes = bytemuck::cast_slice(&records);
        self.global_bind_group_dirty |= d.records.upload(&self.device, &self.queue, bytes);
        self.frame_counters.decal_maps_dropped = dropped;
    }
}

/// The library material `decal` names, if it names one that exists.
fn resolve<'a>(
    materials: &'a BTreeMap<String, MaterialAsset>,
    decal: &Decal,
) -> Option<&'a MaterialAsset> {
    materials.get(decal.material.as_deref()?)
}
