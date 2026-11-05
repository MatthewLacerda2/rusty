//! Shadow casters as instanced draws (#470).
//!
//! Every caster the light can see is gathered as `(mesh, world matrix)`, sorted by mesh
//! so copies of one prop are neighbours, and packed into one matrix array; each run of
//! one mesh is then a single instanced depth draw. The plain depth pass has no
//! material — it writes depth only — so the mesh alone is the key; a clipped caster
//! (#648, `clips`) also keys on its cut and material group, which its draw binds, and
//! every caster on its [`Facing`]: a mirrored one (#1134) flips its winding, a
//! two-sided one (#1571) culls nothing, one whose material draws its back or both
//! faces (#1615) casts them, and each picks its pipeline. A mesh whose
//! `cast_shadows` is `Off` is never gathered.
//!
//! Skinned casters are posed (#599): each sweep packs its skinned casters' palettes
//! (`MeshComponent::active_palette`, the matrices the forward pass draws) into the
//! buffer's joint array with the forward pass's own `push_palette`, and each caster
//! carries its `bone_base` per instance. The base rides the instance, not the draw, so
//! skinned copies of one mesh still share one instanced draw.
//!
//! A caster the camera may cull as too far or too small (#1714, `cull`), and every
//! renderer of its `LODGroup` (`UnbakedCasters`), is never cached in a static bake:
//! it casts from the per-frame sweeps, with the base camera's verdict, so its shadow
//! goes when it does.
//!
//! The casters are the retained scene's objects (`render::scene`, #1706): a sweep
//! walks them, never the ECS, and reads per frame only what the frame decides (the
//! LOD level, the small-object verdict) and a skinned caster's pose.
//!
//! Casters are gathered once per sweep and culled per light volume — a cascade (#435)
//! or an atlas tile (#468) — so a prop inside two volumes is drawn into both. The
//! cascades' and the atlas's static bakes and dynamic passes each own a [`CasterBuffer`]
//! holding every volume's casters: all four are recorded into the same encoder
//! before one submit, so sharing one buffer would let a later upload overwrite an
//! earlier sweep's casters.

mod buffer;
mod cull;
mod runs;
mod sweep;

use std::collections::HashMap;
use std::ops::Range;

use super::clips::Clip;
use super::ShadowRenderer;
use crate::components::{RenderFace, ShadowCasting};
use crate::render::draw::batch::IndexRun;
use crate::render::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::render::gpu::material_cache::MaterialCache;
use crate::render::gpu::pipelines::surface::SurfaceShaders;
use crate::render::gpu::pipelines::winding::Facing;
use crate::render::lod::LodSelection;
use crate::render::scene::{MaterialRecords, MeshHandle, MeshHandles, RenderScene};
use crate::render::{GpuMesh, MeshId, SmallObjectCull};
use crate::scene::Scene;
pub(super) use buffer::CasterBuffer;
use buffer::{CasterData, CasterRecords, CasterUpload};
use cull::Candidate;
pub(crate) use cull::ShadowCulls;
use glam::Mat4;
use runs::batch_casters;
pub(crate) use runs::{shadow_runs, ShadowRun};
pub(super) use sweep::Sweep;

/// The facing a caster placed by `world` draws with: every triangle when it is
/// two-sided (#1571), else its material's render face (#1615, as Unity's
/// ShadowCaster pass culls by `_Cull`), its winding flipped when mirrored (#1134).
pub(super) fn caster_facing(world: Mat4, two_sided: bool, face: RenderFace) -> Facing {
    match two_sided {
        true => Facing::Both,
        false => Facing::of(world, face),
    }
}

/// What a run of casters shares: its facing (mirrored, #1134, or two-sided, #1571),
/// its clip (`None` for plain depth), then its mesh. Field order is sort order:
/// facing first, so one clip pipeline's facings don't alternate per material.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CasterKey {
    pub facing: Facing,
    pub clip: Option<Clip>,
    pub mesh: MeshHandle,
    /// The drawn index run, whole (#1285): material runs that clip differently are
    /// draws of their own, and two casters whose runs of one mesh are joined
    /// differently never share a draw.
    pub indices: IndexRun,
}

/// One instanced depth draw: `instances` of `key`'s mesh, its index run each, and
/// the sweep's material record its clip reads (#1616; the flat one for a draw that
/// reads none).
pub(super) struct CasterBatch {
    key: CasterKey,
    instances: Range<u32>,
    record: u32,
}

impl CasterBatch {
    /// `(num_indices, instances)` — the draw as the frame counters take it (#433).
    pub(super) fn counts(&self) -> (u32, u32) {
        (self.key.indices.len(), self.instances.len() as u32)
    }
}

/// What one depth sweep needs from the renderer.
pub(super) struct CasterFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub scene: &'a Scene,
    /// `scene`'s retained objects (#1706): every caster, with its slots' clips
    /// (#648) and what its static bake reads.
    pub objects: &'a RenderScene,
    pub gpu_meshes: &'a HashMap<MeshId, GpuMesh>,
    pub meshes: &'a MeshHandles,
    /// The material groups and variant pipelines the clipped casters' draws bind.
    pub materials: &'a MaterialCache,
    /// The material table (#1706): a clip's record, read as it is now (a record-only
    /// edit's cut follows), named by its value class ([`Clip::by_class`]).
    pub records: &'a MaterialRecords,
    pub surfaces: &'a SurfaceShaders,
    /// The scene's [`StaticsKey`](super::StaticsKey), kept by the retained scene and
    /// compared by both the cascades' and the atlas's static bakes (#1594).
    pub statics: super::StaticsKey,
    /// The renderer's GPU pass timer: every sweep is a `shadows` pass (#835).
    pub timer: &'a crate::render::timing::GpuTimer,
    /// The base camera's small-object cull (#1714): a shadow goes with its prop.
    pub small: SmallObjectCull,
}

impl CasterFrame<'_> {
    /// The resident buffers of the mesh `handle` names.
    fn gpu_mesh(&self, handle: MeshHandle) -> Option<&GpuMesh> {
        self.gpu_meshes.get(self.meshes.id(handle))
    }
}

impl ShadowRenderer {
    /// Gather every active caster `sweep` takes whose LOD level `lod` shows, cull it
    /// against each of `volumes` (light view-projections), and upload every volume's
    /// batches into that sweep's one caster buffer. Returns the batches per volume,
    /// in the order given.
    pub(super) fn prepare_casters(
        &mut self,
        frame: &CasterFrame,
        lod: &LodSelection,
        sweep: Sweep,
        volumes: &[Mat4],
    ) -> Vec<Vec<CasterBatch>> {
        let (candidates, joints) = self.collect_casters(frame, lod, sweep);
        let mut matrices = Vec::new();
        let mut per_volume = Vec::with_capacity(volumes.len());
        for &volume in volumes {
            let casters = cull::cull(&candidates, volume, &mut self.culls);
            let (packed, mut batches) = batch_casters(casters, self.instancing);
            let base = matrices.len() as u32;
            for batch in &mut batches {
                batch.instances = batch.instances.start + base..batch.instances.end + base;
            }
            matrices.extend(packed);
            self.drawn.extend(batches.iter().map(CasterBatch::counts));
            per_volume.push(batches);
        }
        // Indexed once every volume is batched: each distinct slot its clips name.
        let clips = per_volume.iter().flatten().map(|b| b.key.clip.as_ref());
        let records = CasterRecords::new(clips, frame.records.table());
        for batch in per_volume.iter_mut().flatten() {
            batch.record = records.index(batch.key.clip.as_ref());
        }
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

    /// Every active caster `sweep` takes — a caster a surface variant cuts counts as
    /// dynamic (see `clips`) — that has a GPU mesh, from the retained scene's
    /// objects (#1706), and the joint matrices its skinned casters' `bone_base`s
    /// index — one palette per caster per sweep, however many cascades draw it. Only
    /// what the frame decides is read here: the LOD level, the small-object verdict,
    /// and a palette object's pose and posed bounds, which animation writes unstamped.
    fn collect_casters(
        &self,
        frame: &CasterFrame,
        lod: &LodSelection,
        sweep: Sweep,
    ) -> (Vec<Candidate>, Vec<JointMatrix>) {
        let (mut casters, mut joints) = (Vec::new(), Vec::new());
        for (id, object) in frame.objects.objects() {
            // An inactive caster, one set to cast nothing (#1571), or a level of detail
            // this sweep does not draw (#472).
            let off = !object.cast.casts() || !object.active;
            if !object.resident || off || lod.hides(id) {
                continue;
            }
            // One a camera may cull as small casts per frame, never from a bake (#1714).
            let unbaked = frame
                .objects
                .unbaked
                .holds(id, object.casting.culls_small());
            if !sweep.takes(object.is_static && !object.moving && !unbaked) {
                continue;
            }
            let world = object.world;
            // A skinned caster is bounded by its posed skeleton (#833); one whose pose
            // cannot be bounded is never culled — a wrongly-culled caster drops its shadow.
            let mesh = object.palette.then(|| frame.scene.world.mesh(id)).flatten();
            let bounds = match (&mesh, frame.gpu_mesh(object.mesh)) {
                (Some(mesh), Some(gpu)) => gpu.world_bounds(mesh, world),
                _ => object.bounds,
            };
            let small = frame.culls_small(object, bounds);
            // A culled caster never draws: no palette to upload for it.
            let palette = mesh.as_ref().map_or(&[][..], |m| m.active_palette());
            let bone_base = (!small).then(|| push_palette(&mut joints, palette));
            let bone_base = bone_base.unwrap_or(0);
            let two_sided = object.cast == ShadowCasting::TwoSided;
            for (indices, (clip, face)) in object.shadow_runs.iter().cloned() {
                let mut caster = CasterData::new(world, bone_base);
                if let Some(cut) = clip.and_then(|c| c.cutout) {
                    caster = caster.cutout(cut);
                }
                casters.push(Candidate {
                    id,
                    key: CasterKey {
                        facing: caster_facing(world, two_sided, face),
                        clip: clip.map(|c| c.clip.by_class(frame.records)),
                        mesh: object.mesh,
                        indices: indices.into(),
                    },
                    caster,
                    bounds,
                    small,
                });
            }
        }
        (casters, joints)
    }

    /// Record one sweep's instanced depth draws for its `volume`th light volume from
    /// its caster buffer: plain runs through the depth-only pipeline, clipped ones
    /// through the cutout clip or their variant's cut, with their material group
    /// bound (#648).
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
        render_pass.set_bind_group(0, global, &[offset]);
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
                render_pass.set_bind_group(1, &casters.bind_group, &offsets);
                record = Some(batch.record);
            }
            if let Some(clip) = clip {
                render_pass.set_bind_group(2, frame.materials.group(clip.material), &[]);
            }
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(batch.key.indices.range(), 0, batch.instances.clone());
        }
    }
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod tests;
