mod buffer;
mod cull;
mod gather;
mod reach;
mod runs;
mod sweep;

use std::collections::HashMap;
use std::ops::Range;

use super::clips::Clip;
use super::ShadowRenderer;
use crate::draw::batch::IndexRun;
use crate::gpu::groups;
use crate::gpu::material_cache::MaterialCache;
use crate::gpu::pipelines::surface::SurfaceShaders;
use crate::gpu::pipelines::winding::Facing;
use crate::lod::LodSelection;
use crate::scene::{MaterialRecords, MeshHandle, MeshHandles, RenderScene};
use crate::{GpuMesh, MeshId, SmallObjectCull};
pub(super) use buffer::CasterBuffer;
use buffer::{CasterData, CasterRecords, CasterUpload};
pub(super) use cull::LightVolume;
pub(crate) use cull::ShadowCulls;
use glam::Mat4;
use reach::Reach;
use runs::batch_casters;
pub(crate) use runs::{shadow_runs, ShadowRun};
use rusty_components::RenderFace;
use rusty_scene::Scene;
pub(super) use sweep::Sweep;

pub(super) fn caster_facing(world: Mat4, two_sided: bool, face: RenderFace) -> Facing {
    match two_sided {
        true => Facing::Both,
        false => Facing::of(world, face),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CasterKey {
    pub facing: Facing,
    pub clip: Option<Clip>,
    pub mesh: MeshHandle,
    pub indices: IndexRun,
}

pub(super) struct CasterBatch {
    key: CasterKey,
    instances: Range<u32>,
    record: u32,
}

impl CasterBatch {
    pub(super) fn counts(&self) -> (u32, u32) {
        (self.key.indices.len(), self.instances.len() as u32)
    }
}

pub(super) struct CasterFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub scene: &'a Scene,
    pub objects: &'a RenderScene,
    pub gpu_meshes: &'a HashMap<MeshId, GpuMesh>,
    pub meshes: &'a MeshHandles,
    pub materials: &'a MaterialCache,
    pub records: &'a MaterialRecords,
    pub surfaces: &'a SurfaceShaders,
    pub statics: super::StaticsKey,
    pub timer: &'a crate::timing::GpuTimer,
    pub small: SmallObjectCull,
}

impl CasterFrame<'_> {
    fn gpu_mesh(&self, handle: MeshHandle) -> Option<&GpuMesh> {
        self.gpu_meshes.get(self.meshes.id(handle))
    }
}

impl ShadowRenderer {
    pub(super) fn prepare_casters(
        &mut self,
        frame: &CasterFrame,
        lod: &LodSelection,
        sweep: Sweep,
        volumes: &[LightVolume],
    ) -> Vec<Vec<CasterBatch>> {
        frame.timer.lap("shadows");
        let start = std::time::Instant::now();
        let views: Vec<Mat4> = volumes.iter().map(|v| v.view_proj).collect();
        let (candidates, joints) = match self.index_casters {
            true => {
                let near = frame.objects.casters_near(&views, sweep.baked());
                self.collect_casters(frame, lod, sweep, near)
            }
            false => self.collect_casters(frame, lod, sweep, frame.objects.objects()),
        };
        let (walls, pvs) = (frame.objects.walls(), &frame.objects.occlusion);
        let index = self.index_casters.then(|| frame.objects.caster_index());
        let reach = Reach::new(&candidates, index);
        let mut matrices = Vec::new();
        let mut per_volume = Vec::with_capacity(volumes.len());
        for &volume in volumes {
            let volume = volume.resident(walls);
            let picked = reach.of(volume.view_proj);
            let picked = picked.into_iter().flat_map(|runs| &candidates[runs]);
            let casters = cull::cull(picked, volume, pvs, &mut self.culls);
            let (packed, mut batches) = batch_casters(casters, self.instancing);
            let base = matrices.len() as u32;
            for batch in &mut batches {
                batch.instances = batch.instances.start + base..batch.instances.end + base;
            }
            matrices.extend(packed);
            self.drawn.extend(batches.iter().map(CasterBatch::counts));
            per_volume.push(batches);
        }
        let clips = per_volume.iter().flatten().map(|b| b.key.clip.as_ref());
        let records = CasterRecords::new(clips, frame.records.table());
        for batch in per_volume.iter_mut().flatten() {
            batch.record = records.index(batch.key.clip.as_ref());
        }
        self.culls.gather_us += start.elapsed().as_micros() as u64;
        frame.timer.lap("shadow_gather");
        let buffer = match sweep {
            Sweep::Static => &mut self.static_casters,
            Sweep::Dynamic => &mut self.dynamic_casters,
            Sweep::AtlasStatic => &mut self.atlas.static_casters,
            Sweep::AtlasDynamic => &mut self.atlas.dynamic_casters,
        };
        let layout = &self.entity_layout;
        let sweep = CasterUpload {
            casters: &matrices,
            joints: &joints,
            records: &records,
        };
        buffer.upload(frame.device, frame.queue, layout, sweep);
        per_volume
    }

    pub(super) fn draw_casters<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frame: &CasterFrame<'a>,
        batches: &[CasterBatch],
        sweep: Sweep,
        volume: usize,
    ) {
        let (casters, global) = match sweep {
            Sweep::Static => (&self.static_casters, &self.global_bind_group),
            Sweep::Dynamic => (&self.dynamic_casters, &self.global_bind_group),
            Sweep::AtlasStatic => (&self.atlas.static_casters, &self.atlas.global),
            Sweep::AtlasDynamic => (&self.atlas.dynamic_casters, &self.atlas.global),
        };
        let offset = (volume as u64 * super::LIGHT_SPACE_STRIDE) as u32;
        render_pass.set_bind_group(groups::GLOBAL, global, &[offset]);
        let (mut bound, mut record) = (None, None);
        for batch in batches {
            let Some(gpu_mesh) = frame.gpu_mesh(batch.key.mesh) else {
                continue;
            };
            let (clip, facing) = (batch.key.clip, batch.key.facing);
            let wanted = (clip.map(|c| c.pipeline), facing);
            if bound != Some(wanted) {
                render_pass.set_pipeline(self.pipeline_for(frame.surfaces, clip, facing));
                bound = Some(wanted);
            }
            if record != Some(batch.record) {
                let offsets = CasterBuffer::offsets(batch.record);
                render_pass.set_bind_group(groups::DRAW, &casters.bind_group, &offsets);
                record = Some(batch.record);
            }
            if let Some(clip) = clip {
                render_pass.set_bind_group(
                    groups::MATERIAL,
                    frame.materials.group(clip.material),
                    &[],
                );
            }
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(batch.key.indices.range(), 0, batch.instances.clone());
        }
    }
}

#[cfg(test)]
mod cull_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod tests;
