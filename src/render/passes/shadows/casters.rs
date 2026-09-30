//! Shadow casters as instanced draws (#470).
//!
//! Every caster the light can see is gathered as `(mesh, world matrix)`, sorted by mesh
//! so copies of one prop are neighbours, and packed into one matrix array; each run of
//! one mesh is then a single instanced depth draw. The depth pass has no material — it
//! writes depth only, and `shadow.wgsl` does not skin — so the mesh alone is the key.
//!
//! Casters are gathered once per sweep and culled per cascade (#435), so a prop inside
//! two cascades is drawn into both. The static bake and the dynamic pass each own a
//! [`CasterBuffer`] holding every cascade's matrices: both are recorded into the same
//! encoder before one submit, so sharing one buffer would let the second upload
//! overwrite the first's matrices.

use std::collections::HashMap;
use std::ops::Range;

use glam::Mat4;

use super::ShadowRenderer;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::{transform_aabb, Frustum, GpuMesh, MeshId};
use crate::scene::Scene;

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

/// A caster matrix array and the bind group that exposes it to `shadow.wgsl`.
pub(super) struct CasterBuffer {
    matrices: GrowBuffer,
    bind_group: wgpu::BindGroup,
}

impl CasterBuffer {
    pub(super) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let matrices = GrowBuffer::new(
            device,
            "Shadow Caster Matrices",
            wgpu::BufferUsages::STORAGE,
        );
        let bind_group = caster_group(device, layout, matrices.buffer());
        Self {
            matrices,
            bind_group,
        }
    }

    /// Upload `matrices`, rebuilding the bind group if the buffer had to grow.
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        matrices: &[[f32; 16]],
    ) {
        if self
            .matrices
            .upload(device, queue, bytemuck::cast_slice(matrices))
        {
            self.bind_group = caster_group(device, layout, self.matrices.buffer());
        }
    }
}

fn caster_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Caster Bind Group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

/// Group `casters` into instanced runs of one mesh, returning the packed matrices (in
/// draw order) and the batches. Sorted by mesh — stable, so scene order holds within
/// a run; depth writes make the order between runs irrelevant. With `instancing` off,
/// one draw per caster in scene order (the pre-#470 path, for comparison).
pub(super) fn batch_casters(
    mut casters: Vec<(MeshId, u32, Mat4)>,
    instancing: bool,
) -> (Vec<[f32; 16]>, Vec<CasterBatch>) {
    if instancing {
        casters.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let mut matrices = Vec::with_capacity(casters.len());
    let mut batches: Vec<CasterBatch> = Vec::new();
    for (mesh, num_indices, world) in casters {
        let index = matrices.len() as u32;
        matrices.push(world.to_cols_array());
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
        let candidates = self.collect_casters(frame, want_static);
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
                .map(|c| (c.mesh.clone(), c.num_indices, c.world))
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
        buffer.upload(frame.device, frame.queue, &self.entity_layout, &matrices);
        per_cascade
    }

    /// Every active caster matching `want_static` that has a GPU mesh.
    fn collect_casters(&self, frame: &CasterFrame, want_static: bool) -> Vec<Candidate> {
        let scene = frame.scene;
        let mut casters = Vec::new();
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
            casters.push(Candidate {
                mesh: mesh_id,
                num_indices: gpu_mesh.num_indices,
                world,
                bounds,
            });
        }
        casters
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

/// One caster before culling: its draw and its world bounds (`None` when skinned —
/// never culled).
struct Candidate {
    mesh: MeshId,
    num_indices: u32,
    world: Mat4,
    bounds: Option<(glam::Vec3, glam::Vec3)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casters_of_one_mesh_become_one_instanced_draw() {
        let at = |x: f32| Mat4::from_translation(glam::Vec3::X * x);
        let mesh = |m: &str| MeshId(m.to_string());
        let casters = vec![
            (mesh("Box"), 36, at(0.0)),
            (mesh("Pillar"), 96, at(1.0)),
            (mesh("Box"), 36, at(2.0)),
        ];
        let (matrices, batches) = batch_casters(casters.clone(), true);
        let counts: Vec<_> = batches.iter().map(CasterBatch::counts).collect();
        assert_eq!(counts, [(36, 2), (96, 1)]);
        let xs: Vec<f32> = matrices.iter().map(|m| m[12]).collect();
        assert_eq!(xs, [0.0, 2.0, 1.0], "a run keeps scene order");
        let (_, solo) = batch_casters(casters, false);
        assert_eq!(solo.len(), 3, "instancing off: one draw per caster");
    }
}
