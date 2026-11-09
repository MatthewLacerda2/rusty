//! Recording a camera's solid batches into a pass (#470): one instanced draw per
//! batch, or one indirect draw per batch over the instances GPU occlusion culling
//! kept (#1963), a clustered batch one indirect draw per instance over the
//! clusters it kept (#1968). The opaque batches are an [`OpaqueBatches`], drawn only by
//! [`Renderer::draw_solids`]; [`Renderer::draw_batches`] takes the translucents'
//! slices, so the compiler keeps an opaque pass off the uncull path.

use crate::render::draw::batch::{DrawBatch, OpaqueBatches};
use crate::render::gpu::draw_buffers::DrawBuffers;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::passes::hiz::{Culled, ARGS};
use crate::render::Renderer;

/// A camera's opaque batches, and the GPU occlusion cull's draws of them when its
/// stage ran (#1963).
pub(crate) type Solids<'a> = (&'a OpaqueBatches, Option<&'a Culled>);

impl Renderer {
    /// Record `batches` into `pass`: per batch, its pipeline when it differs from the
    /// last one (see [`SolidPass`]), the mesh buffers, the frame's group-1 bind group
    /// at the batch's uniform offset, the shared material group, and one instanced
    /// draw over its instance range (#470). The translucent passes' path: the
    /// opaque batches are an [`OpaqueBatches`], which this cannot take, and draw
    /// through [`Self::draw_solids`].
    pub(crate) fn draw_batches<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        batches: &'a [DrawBatch],
        solid_pass: SolidPass<'a>,
    ) {
        self.record_batches(render_pass, (batches.iter(), None), solid_pass);
    }

    /// The opaque batches, as [`Self::draw_batches`] records batches, but through
    /// the GPU occlusion cull's kept instances and indirect args when it ran
    /// (#1963), batch `i` drawing the args at `i`. The one way the prepass, the
    /// forward and velocity pass and phase one draw them.
    pub(crate) fn draw_solids<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        (batches, culled): Solids<'a>,
        solid_pass: SolidPass<'a>,
    ) {
        self.record_batches(render_pass, (batches.iter(), culled), solid_pass);
    }

    fn record_batches<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        (batches, culled): (impl Iterator<Item = &'a DrawBatch>, Option<&'a Culled>),
        solid_pass: SolidPass<'a>,
    ) {
        let mut bound = None;
        let group = culled.map_or(self.draw_buffers.bind_group(), |c| &c.group);
        for (i, batch) in batches.enumerate() {
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
            let by_cluster = culled
                .and_then(|c| c.clusters.as_ref())
                .and_then(|c| Some((c, c.draws.get(i)?.clone()?)));
            match (culled, by_cluster) {
                (_, Some((c, jobs))) => {
                    // A clustered batch (#1968): each instance's kept clusters.
                    render_pass.set_index_buffer(c.indices.slice(..), wgpu::IndexFormat::Uint32);
                    for job in jobs {
                        render_pass.draw_indexed_indirect(&c.args, ARGS * u64::from(job));
                    }
                }
                (Some(c), None) => render_pass.draw_indexed_indirect(&c.args, ARGS * i as u64),
                (None, None) => {
                    render_pass.draw_indexed(batch.key.indices.range(), 0, batch.instances.clone())
                }
            }
        }
    }
}
