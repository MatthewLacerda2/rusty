//! Recording a camera's solid batches into a pass (#470): one instanced draw per
//! batch, or one indirect draw per batch over the instances GPU occlusion culling
//! kept (#1963), a clustered batch one indirect draw per instance over the
//! clusters it kept (#1968). The opaque batches are an [`OpaqueBatches`], drawn only by
//! [`Renderer::draw_solids`]; [`Renderer::draw_batches`] takes the translucents'
//! slices, so the compiler keeps an opaque pass off the uncull path.

use crate::draw::batch::{DrawBatch, OpaqueBatches};
use crate::gpu::draw_buffers::DrawBuffers;
use crate::gpu::pipelines::surface::SolidPass;
use crate::passes::hiz::{Culled, ARGS};
use crate::Renderer;

/// A camera's opaque batches, and the GPU occlusion cull's draws of them when its
/// stage ran (#1963).
pub(crate) type Solids<'a> = (&'a OpaqueBatches, Option<&'a Culled>);

impl Renderer {
    /// Record `batches` into `pass`: per batch, its pipeline when it differs from the
    /// last one (see [`SolidPass`]), the mesh buffers, the frame's group-1 bind group
    /// at the batch's uniform offset, the shared material group (but for a prepass
    /// batch nothing clips, whose pipeline has no fragment stage, #2347), and one instanced
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
    /// (#1963), batch `i` drawing the args at `i` from the draws' start. The one
    /// way the prepass, the forward and velocity pass and phase one draw them.
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
            // A batch nothing clips draws the prepass with no fragment stage (#2347).
            let plain = batch.key.depth_only.then(|| solid_pass.depth_only(facing));
            let depth_only = plain.flatten();
            let pipeline = (
                batch.key.pipeline,
                depth_only.is_some(),
                primed,
                blend,
                facing,
            );
            if bound != Some(pipeline) {
                let pick = depth_only.unwrap_or_else(|| {
                    self.surface_shaders
                        .pick(solid_pass, batch.key.pipeline, primed, blend, facing)
                });
                render_pass.set_pipeline(pick);
                bound = Some(pipeline);
                #[cfg(test)]
                probe::set(solid_pass, pick);
            }
            let offsets = DrawBuffers::offsets(batch.uniform_slot);
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, group, &offsets);
            if depth_only.is_none() {
                render_pass.set_bind_group(2, self.materials.group(batch.key.material), &[]);
            }
            #[cfg(test)]
            probe::draw(solid_pass, depth_only.is_none());
            Self::issue(render_pass, culled, i, batch);
        }
    }

    /// Batch `i`'s draw: by cluster, through the cull's indirect args, or as kept.
    fn issue<'a>(
        render_pass: &mut wgpu::RenderPass<'a>,
        culled: Option<&'a Culled>,
        i: usize,
        batch: &DrawBatch,
    ) {
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
            (Some(c), None) => render_pass.draw_indexed_indirect(&c.args, c.at + ARGS * i as u64),
            (None, None) => {
                render_pass.draw_indexed(batch.key.indices.range(), 0, batch.instances.clone())
            }
        }
    }
}

/// What the depth prepass recorded (#2347), a test's evidence that a batch nothing
/// clips draws through the fragment-less pipeline and binds no material: counted
/// per thread from the pipeline each draw was recorded with, compared by identity
/// with the pass's own depth-only ones, so a wrong pick shows even when the batch's
/// key says depth-only. Test code only.
#[cfg(test)]
pub(crate) mod probe {
    use super::SolidPass;
    use crate::gpu::pipelines::Facing;
    use std::cell::Cell;

    /// The prepass draws since the last [`take`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(crate) struct PrepassDraws {
        /// Draws recorded in a prepass.
        pub draws: u32,
        /// Of those, the ones whose pipeline was a fragment-less one.
        pub depth_only: u32,
        /// Of those, the ones that bound a material group.
        pub material_binds: u32,
    }

    thread_local! {
        static COUNTS: Cell<PrepassDraws> = const { Cell::new(PrepassDraws {
            draws: 0,
            depth_only: 0,
            material_binds: 0,
        }) };
        /// Whether the pipeline last set in a prepass is a fragment-less one.
        static PLAIN: Cell<bool> = const { Cell::new(false) };
    }

    /// Note `pipeline`, just set in `pass`.
    pub(super) fn set(pass: SolidPass<'_>, pipeline: &wgpu::RenderPipeline) {
        // Only the built ones: comparing must build no pipeline.
        let plain = match pass {
            SolidPass::Prepass(p) => Facing::ALL
                .into_iter()
                .any(|f| p.depth_only.is_built(f) && std::ptr::eq(p.depth_only.get(f), pipeline)),
            _ => false,
        };
        PLAIN.with(|c| c.set(plain));
    }

    /// Count one draw of `pass`, which bound its material group when `material`.
    pub(super) fn draw(pass: SolidPass<'_>, material: bool) {
        if !pass.is_prepass() {
            return;
        }
        let plain = PLAIN.with(Cell::get);
        COUNTS.with(|c| {
            let mut n = c.get();
            n.draws += 1;
            n.depth_only += u32::from(plain);
            n.material_binds += u32::from(material);
            c.set(n);
        });
    }

    /// What this thread counted since the last call, and start again from zero.
    pub(crate) fn take() -> PrepassDraws {
        COUNTS.with(Cell::take)
    }
}
