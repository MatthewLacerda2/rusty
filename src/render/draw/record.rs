//! Recording a camera's solid batches into a pass (#470): one instanced draw per
//! batch, or one indirect draw per batch over the instances GPU occlusion culling
//! kept (#1963).

use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::draw_buffers::DrawBuffers;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::passes::hiz::{Culled, ARGS};
use crate::render::Renderer;

/// A camera's opaque batches, and the GPU occlusion cull's draws of them when its
/// stage ran (#1963).
pub(crate) type Solids<'a> = (&'a [DrawBatch], Option<&'a Culled>);

impl Renderer {
    /// Record `batches` into `pass`: per batch, its pipeline when it differs from the
    /// last one (see [`SolidPass`]), the mesh buffers, the frame's group-1 bind group
    /// at the batch's uniform offset, the shared material group, and one instanced
    /// draw over its instance range (#470). Shared by the prepass, opaque and
    /// transparent passes.
    pub(crate) fn draw_batches<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        batches: &'a [DrawBatch],
        solid_pass: SolidPass<'a>,
    ) {
        self.draw_solids(render_pass, (batches, None), solid_pass);
    }

    /// [`Self::draw_batches`] of `solids`: through the GPU occlusion cull's kept
    /// instances and indirect args when it ran (#1963), batch `i` drawing the
    /// args at `i`.
    pub(crate) fn draw_solids<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        (batches, culled): Solids<'a>,
        solid_pass: SolidPass<'a>,
    ) {
        let mut bound = None;
        let group = culled.map_or(self.draw_buffers.bind_group(), |c| &c.group);
        for (i, batch) in batches.iter().enumerate() {
            let Some(gpu_mesh) = self.gpu_mesh_of(batch.key.mesh) else {
                continue;
            };
            let in_prepass = batch.key.in_prepass();
            if solid_pass.is_prepass() && !in_prepass {
                continue;
            }
            let primed = solid_pass.primed() && in_prepass;
            let blend = batch.key.traits.blend;
            let facing = batch.key.facing;
            let pipeline = (batch.key.pipeline, primed, blend, facing);
            if bound != Some(pipeline) {
                let pick = self
                    .surface_shaders
                    .pick(solid_pass, pipeline.0, primed, blend, facing);
                render_pass.set_pipeline(pick);
                bound = Some(pipeline);
            }
            let offsets = DrawBuffers::offsets(batch.uniform_slot);
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, group, &offsets);
            render_pass.set_bind_group(2, self.materials.group(batch.key.material), &[]);
            match culled {
                Some(c) => render_pass.draw_indexed_indirect(&c.args, ARGS * i as u64),
                None => {
                    render_pass.draw_indexed(batch.key.indices.range(), 0, batch.instances.clone())
                }
            }
        }
    }
}
