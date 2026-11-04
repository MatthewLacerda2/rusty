//! The frame's decals on the GPU (#638): one record array uploaded once per frame,
//! and the atlas their maps live in, bound in group 0 at bindings 10–13. Each
//! camera bins the same records into its clusters with its lights
//! (`Renderer::bin_clusters`).

use std::collections::BTreeMap;

use glam::Vec3;

use super::atlas::DecalAtlas;
use super::kept::KeptDecals;
use super::record::{self, GpuDecal, NO_LAYER};
use crate::asset::MapRef;
use crate::components::MaterialAsset;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::gpu::textures::TextureKind;
use crate::render::Renderer;
use crate::scene::decal::{Decal, DecalPose};
use crate::scene::Scene;

pub(crate) struct DecalBuffers {
    records: GrowBuffer,
    pub(crate) atlas: DecalAtlas,
    /// The frame's decals' bounding spheres, in record order, for binning.
    pub(crate) spheres: Vec<(Vec3, f32)>,
    /// Each decal's maps and the records uploaded, kept between frames.
    kept: KeptDecals,
}

impl DecalBuffers {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            records: GrowBuffer::new(device, "Decal Records", wgpu::BufferUsages::STORAGE),
            atlas: DecalAtlas::new(device),
            spheres: Vec::new(),
            kept: KeptDecals::default(),
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
    /// through its world matrix (`World::local_to_world`, never stale), and one
    /// whose owner is inactive or that has faded out is left out. Each decal's maps
    /// are kept from frame to frame, and records equal to the uploaded ones are not
    /// written again (#1706, `kept`).
    pub(crate) fn upload_decals(&mut self, scene: &Scene) {
        let d = &mut self.decals;
        d.kept.begin(scene);
        let drawn: Vec<(&Decal, DecalPose)> = scene
            .decals
            .iter()
            .filter_map(|decal| Some((decal, scene.decal_pose(decal)?)))
            .collect();
        for (decal, _) in &drawn {
            d.kept.read(scene, decal);
        }
        let maps: Vec<(&MapRef, TextureKind)> = drawn
            .iter()
            .flat_map(|(decal, _)| record::kinded(d.kept.of(decal.id)))
            .collect();
        let misses = &mut self.texture_freshness;
        let gpu = (&self.device, &self.queue, &self.mip_generator);
        self.global_bind_group_dirty |= d.atlas.prepare(gpu, &maps, misses);

        let mut dropped = 0;
        let records: Vec<GpuDecal> = drawn
            .iter()
            .map(|(decal, pose)| {
                let maps = d.kept.of(decal.id);
                let layers = std::array::from_fn(|i| {
                    let Some(path) = &maps[i] else {
                        return NO_LAYER;
                    };
                    let layer = d.atlas.layer(path.as_str());
                    dropped += u32::from(layer == NO_LAYER && !misses.is_miss(path.as_str()));
                    layer
                });
                record::record(decal, pose, resolve(&scene.materials, decal), layers)
            })
            .collect();
        d.spheres = drawn.iter().map(|(_, pose)| record::bounds(pose)).collect();
        if records != d.kept.uploaded {
            let bytes = bytemuck::cast_slice(&records);
            self.global_bind_group_dirty |= d.records.upload(&self.device, &self.queue, bytes);
            d.kept.uploaded = records;
        }
        d.kept.end();
        self.frame_counters.decal_maps_dropped = dropped;
    }
}

/// The library material `decal` names, if it names one that exists.
pub(super) fn resolve<'a>(
    materials: &'a BTreeMap<String, MaterialAsset>,
    decal: &Decal,
) -> Option<&'a MaterialAsset> {
    materials.get(decal.material.as_deref()?)
}
