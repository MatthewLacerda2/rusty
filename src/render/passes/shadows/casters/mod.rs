//! Shadow casters as instanced draws (#470).
//!
//! Every caster the light can see is gathered as `(mesh, world matrix)`, sorted by mesh
//! so copies of one prop are neighbours, and packed into one matrix array; each run of
//! one mesh is then a single instanced depth draw. The plain depth pass has no
//! material — it writes depth only — so the mesh alone is the key; a clipped caster
//! (#648, `clips`) also keys on its cut and material group, which its draw binds, and
//! every caster on its [`Facing`]: a mirrored one (#1134) flips its winding, a
//! two-sided one (#1571) culls nothing, and each picks its pipeline. A mesh whose
//! `cast_shadows` is `Off` is never gathered.
//!
//! Skinned casters are posed (#599): each sweep packs its skinned casters' palettes
//! (`MeshComponent::active_palette`, the matrices the forward pass draws) into the
//! buffer's joint array with the forward pass's own `push_palette`, and each caster
//! carries its `bone_base` per instance. The base rides the instance, not the draw, so
//! skinned copies of one mesh still share one instanced draw.
//!
//! Casters are gathered once per sweep and culled per light volume — a cascade (#435)
//! or an atlas tile (#468) — so a prop inside two volumes is drawn into both. The
//! cascades' and the atlas's static bakes and dynamic passes each own a [`CasterBuffer`]
//! holding every volume's casters: all four are recorded into the same encoder
//! before one submit, so sharing one buffer would let a later upload overwrite an
//! earlier sweep's casters.

mod buffer;
mod runs;
mod sweep;

use std::collections::HashMap;
use std::ops::Range;

use super::clips::{CasterClip, Clip};
use super::ShadowRenderer;
use crate::components::ShadowCasting;
use crate::render::draw::batch::IndexRun;
use crate::render::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::render::gpu::material_cache::MaterialCache;
use crate::render::gpu::pipelines::surface::SurfaceShaders;
use crate::render::gpu::pipelines::winding::Facing;
use crate::render::lod::LodSelection;
use crate::render::{Frustum, GpuMesh, MeshId};
use crate::scene::Scene;
pub(super) use buffer::CasterBuffer;
use buffer::{CasterData, CasterRecords, CasterUpload};
use glam::Mat4;
use runs::{batch_casters, join_alike};
pub(super) use sweep::Sweep;

/// What a run of casters shares: its facing (mirrored, #1134, or two-sided, #1571),
/// its clip (`None` for plain depth), then its mesh. Field order is sort order:
/// facing first, so one clip pipeline's facings don't alternate per material.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CasterKey {
    pub facing: Facing,
    pub clip: Option<Clip>,
    pub mesh: MeshId,
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
    pub gpu_meshes: &'a HashMap<MeshId, GpuMesh>,
    /// The clipped casters (#648), and the material groups and variant pipelines
    /// their draws bind.
    pub clips: &'a HashMap<(u32, usize), CasterClip>,
    pub materials: &'a MaterialCache,
    pub surfaces: &'a SurfaceShaders,
    /// The scene's [`StaticsKey`](super::StaticsKey), digested once for the frame
    /// and compared by both the cascades' and the atlas's static bakes (#1594).
    pub statics: super::StaticsKey,
    /// The renderer's GPU pass timer: every sweep is a `shadows` pass (#835).
    pub timer: &'a crate::render::timing::GpuTimer,
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
            // Cull casters against the LIGHT's volume, not the camera's (#330). An
            // off-screen caster still inside a cascade must keep its shadow — culling
            // casters by the player camera is the classic pop-a-shadow bug.
            let frustum = Frustum::from_view_proj(volume);
            let casters = candidates
                .iter()
                .filter(|c| {
                    c.bounds
                        .is_none_or(|(lo, hi)| frustum.intersects_aabb(lo, hi))
                })
                .map(|c| (c.key.clone(), c.caster))
                .collect();
            let (packed, mut batches) = batch_casters(casters, self.instancing);
            let base = matrices.len() as u32;
            for batch in &mut batches {
                batch.instances = batch.instances.start + base..batch.instances.end + base;
            }
            matrices.extend(packed);
            self.drawn.extend(batches.iter().map(CasterBatch::counts));
            per_volume.push(batches);
        }
        // Indexed once every volume is batched: the records sort by their words.
        let records = CasterRecords::new(per_volume.iter().flatten().map(|b| b.key.clip.as_ref()));
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
    /// dynamic (see `clips`) — that has a GPU mesh, and the joint
    /// matrices its skinned casters' `bone_base`s index — one palette per caster per
    /// sweep, however many cascades draw it.
    fn collect_casters(
        &self,
        frame: &CasterFrame,
        lod: &LodSelection,
        sweep: Sweep,
    ) -> (Vec<Candidate>, Vec<JointMatrix>) {
        let scene = frame.scene;
        let mut casters = Vec::new();
        let mut joints = Vec::new();
        for id in scene.world.ids_with_mesh() {
            let mesh = scene.world.mesh(id).expect("id came from ids_with_mesh");
            let mesh_id = MeshId::from_mesh(&mesh);
            let Some(gpu_mesh) = frame.gpu_meshes.get(&mesh_id) else {
                continue;
            };
            // An inactive caster, one set to cast nothing (#1571), or a level of detail
            // this sweep does not draw (#472).
            let off = !mesh.cast_shadows.casts();
            if off || !scene.world.is_active_in_hierarchy(id) || lod.hides(id) {
                continue;
            }
            let material = scene.world.material(id);
            let slots = material.as_ref().map_or(0, |m| m.materials.len());
            let moving = (0..slots).any(|slot| {
                frame
                    .clips
                    .get(&(id, slot))
                    .is_some_and(|c| c.clip.is_variant())
            });
            if !sweep.takes(scene.world.is_static(id) && !moving) {
                continue;
            }
            let runs = gpu_mesh.runs(&mesh, material.as_deref());
            let runs = join_alike(runs, |slot| frame.clips.get(&(id, slot)));
            let world = scene.world.local_to_world(id);
            // A skinned caster is bounded by its posed skeleton (#833); one whose pose
            // cannot be bounded is never culled — a wrongly-culled caster drops its shadow.
            let bounds = gpu_mesh.world_bounds(&mesh, world);
            let bone_base = push_palette(&mut joints, mesh.active_palette());
            let two_sided = mesh.cast_shadows == ShadowCasting::TwoSided;
            for (indices, clip) in runs {
                let mut caster = CasterData::new(world, bone_base);
                if let Some(cut) = clip.and_then(|c| c.cutout) {
                    caster = caster.cutout(cut);
                }
                casters.push(Candidate {
                    key: CasterKey {
                        facing: Facing::of(world, two_sided),
                        clip: clip.map(|c| c.clip),
                        mesh: mesh_id.clone(),
                        indices: indices.into(),
                    },
                    caster,
                    bounds,
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
            let Some(gpu_mesh) = frame.gpu_meshes.get(&batch.key.mesh) else {
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

/// One caster before culling: its draw, its instance data and its world bounds (`None`
/// when they cannot be bounded — never culled).
struct Candidate {
    key: CasterKey,
    caster: CasterData,
    bounds: Option<(glam::Vec3, glam::Vec3)>,
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod tests;
