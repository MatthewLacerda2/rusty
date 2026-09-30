//! Shadow casters as instanced draws (#470).
//!
//! Every caster the light can see is gathered as `(mesh, world matrix)`, sorted by mesh
//! so copies of one prop are neighbours, and packed into one matrix array; each run of
//! one mesh is then a single instanced depth draw. The depth pass has no material — it
//! writes depth only — so the mesh alone is the key.
//!
//! Skinned casters are posed (#599): each sweep packs its skinned casters' palettes
//! (`MeshComponent::active_palette`, the matrices the forward pass draws) into the
//! buffer's joint array with the forward pass's own `push_palette`, and each caster
//! carries its `bone_base` per instance. The base rides the instance, not the draw, so
//! skinned copies of one mesh still share one instanced draw.
//!
//! Casters are gathered once per sweep and culled per cascade (#435), so a prop inside
//! two cascades is drawn into both. The static bake and the dynamic pass each own a
//! [`CasterBuffer`] holding every cascade's casters: both are recorded into the same
//! encoder before one submit, so sharing one buffer would let the second upload
//! overwrite the first's casters.

mod buffer;

use std::collections::HashMap;
use std::ops::Range;

use super::ShadowRenderer;
use crate::render::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::render::{transform_aabb, Frustum, GpuMesh, MeshId};
use crate::scene::Scene;
pub(super) use buffer::CasterBuffer;
use buffer::CasterData;

/// One instanced depth draw: `instances` of `mesh`, `num_indices` each.
pub(super) struct CasterBatch {
    mesh: MeshId,
    num_indices: u32,
    instances: Range<u32>,
}

impl CasterBatch {
    /// `(num_indices, instances)` — the draw as the frame counters take it (#433).
    pub(super) fn counts(&self) -> (u32, u32) {
        (self.num_indices, self.instances.len() as u32)
    }
}

/// Group `casters` into instanced runs of one mesh, returning the packed casters (in
/// draw order) and the batches. Sorted by mesh — stable, so scene order holds within
/// a run; depth writes make the order between runs irrelevant. With `instancing` off,
/// one draw per caster in scene order (the pre-#470 path, for comparison).
pub(super) fn batch_casters(
    mut casters: Vec<(MeshId, u32, CasterData)>,
    instancing: bool,
) -> (Vec<CasterData>, Vec<CasterBatch>) {
    if instancing {
        casters.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let mut matrices = Vec::with_capacity(casters.len());
    let mut batches: Vec<CasterBatch> = Vec::new();
    for (mesh, num_indices, caster) in casters {
        let index = matrices.len() as u32;
        matrices.push(caster);
        match batches.last_mut() {
            Some(run) if instancing && run.mesh == mesh => run.instances.end = index + 1,
            _ => batches.push(CasterBatch {
                mesh,
                num_indices,
                instances: index..index + 1,
            }),
        }
    }
    (matrices, batches)
}

/// What one depth sweep needs from the renderer.
pub(super) struct CasterFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub scene: &'a Scene,
    pub gpu_meshes: &'a HashMap<MeshId, GpuMesh>,
}

impl ShadowRenderer {
    /// Gather every active caster whose `is_static` flag matches `want_static`, cull
    /// it against each of `cascades`' light volumes, and upload every cascade's
    /// batches into that sweep's one caster buffer. Returns the batches per cascade,
    /// in the order given.
    pub(super) fn prepare_casters(
        &mut self,
        frame: &CasterFrame,
        want_static: bool,
        cascades: &[usize],
    ) -> Vec<Vec<CasterBatch>> {
        let (candidates, joints) = self.collect_casters(frame, want_static);
        let mut matrices = Vec::new();
        let mut per_cascade = Vec::with_capacity(cascades.len());
        for &i in cascades {
            // Cull casters against the LIGHT's volume, not the camera's (#330). An
            // off-screen caster still inside a cascade must keep its shadow — culling
            // casters by the player camera is the classic pop-a-shadow bug.
            let frustum = Frustum::from_view_proj(self.cascades[i].light_space);
            let casters = candidates
                .iter()
                .filter(|c| {
                    c.bounds
                        .is_none_or(|(lo, hi)| frustum.intersects_aabb(lo, hi))
                })
                .map(|c| (c.mesh.clone(), c.num_indices, c.caster))
                .collect();
            let (packed, mut batches) = batch_casters(casters, self.instancing);
            let base = matrices.len() as u32;
            for batch in &mut batches {
                batch.instances = batch.instances.start + base..batch.instances.end + base;
            }
            matrices.extend(packed);
            self.drawn.extend(batches.iter().map(CasterBatch::counts));
            per_cascade.push(batches);
        }
        let buffer = if want_static {
            &mut self.static_casters
        } else {
            &mut self.dynamic_casters
        };
        let layout = &self.entity_layout;
        buffer.upload(frame.device, frame.queue, layout, &matrices, &joints);
        per_cascade
    }

    /// Every active caster matching `want_static` that has a GPU mesh, and the joint
    /// matrices its skinned casters' `bone_base`s index — one palette per caster per
    /// sweep, however many cascades draw it.
    fn collect_casters(
        &self,
        frame: &CasterFrame,
        want_static: bool,
    ) -> (Vec<Candidate>, Vec<JointMatrix>) {
        let scene = frame.scene;
        let mut casters = Vec::new();
        let mut joints = Vec::new();
        for id in scene.world.ids_with_mesh() {
            if !scene.world.is_active(id) || scene.world.is_static(id) != want_static {
                continue;
            }
            let mesh = scene.world.mesh(id).expect("id came from ids_with_mesh");
            let mesh_id = MeshId::from_mesh(&mesh);
            let Some(gpu_mesh) = frame.gpu_meshes.get(&mesh_id) else {
                continue;
            };
            let world = scene.world_matrix(id);
            // Skinned casters are never culled — their AABB is the rest pose, which an
            // animation can exceed; a wrongly-culled caster would drop its shadow (#330).
            let bounds = (!mesh.is_skinned())
                .then(|| transform_aabb(gpu_mesh.local_aabb.0, gpu_mesh.local_aabb.1, world));
            let bone_base = push_palette(&mut joints, mesh.active_palette());
            casters.push(Candidate {
                mesh: mesh_id,
                num_indices: gpu_mesh.num_indices,
                caster: CasterData::new(world, bone_base),
                bounds,
            });
        }
        (casters, joints)
    }

    /// Record one sweep's instanced depth draws for `cascade` from its caster buffer.
    pub(super) fn draw_casters<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        gpu_meshes: &'a HashMap<MeshId, GpuMesh>,
        batches: &[CasterBatch],
        want_static: bool,
        cascade: usize,
    ) {
        let casters = if want_static {
            &self.static_casters
        } else {
            &self.dynamic_casters
        };
        let offset = (cascade as u64 * super::LIGHT_SPACE_STRIDE) as u32;
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.global_bind_group, &[offset]);
        render_pass.set_bind_group(1, &casters.bind_group, &[]);
        for batch in batches {
            let Some(gpu_mesh) = gpu_meshes.get(&batch.mesh) else {
                continue;
            };
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..batch.num_indices, 0, batch.instances.clone());
        }
    }
}

/// One caster before culling: its draw, its instance data and its world bounds (`None` when skinned —
/// never culled).
struct Candidate {
    mesh: MeshId,
    num_indices: u32,
    caster: CasterData,
    bounds: Option<(glam::Vec3, glam::Vec3)>,
}

#[cfg(test)]
mod tests;
