use crate::draw::batch::{DrawBatch, OpaqueBatches};
use crate::gpu::draw_buffers::DrawBuffers;
use crate::gpu::groups;
use crate::gpu::pipelines::surface::SolidPass;
use crate::passes::hiz::{Culled, ARGS};
use crate::Renderer;

pub(crate) type Solids<'a> = (&'a OpaqueBatches, Option<&'a Culled>);

impl Renderer {
    pub(crate) fn draw_batches<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        batches: &'a [DrawBatch],
        solid_pass: SolidPass<'a>,
    ) {
        self.record_batches(render_pass, (batches.iter(), None), solid_pass);
    }

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
            render_pass.set_bind_group(groups::DRAW, group, &offsets);
            if depth_only.is_none() {
                render_pass.set_bind_group(
                    groups::MATERIAL,
                    self.materials.group(batch.key.material),
                    &[],
                );
            }
            #[cfg(test)]
            probe::draw(solid_pass, depth_only.is_none());
            Self::issue(render_pass, culled, i, batch);
        }
    }

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

#[cfg(test)]
pub(crate) mod probe {
    use super::SolidPass;
    use crate::gpu::pipelines::Facing;
    use std::cell::Cell;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(crate) struct PrepassDraws {
        pub draws: u32,
        pub depth_only: u32,
        pub material_binds: u32,
    }

    thread_local! {
        static COUNTS: Cell<PrepassDraws> = const { Cell::new(PrepassDraws {
            draws: 0,
            depth_only: 0,
            material_binds: 0,
        }) };
        static PLAIN: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn set(pass: SolidPass<'_>, pipeline: &wgpu::RenderPipeline) {
        let plain = match pass {
            SolidPass::Prepass(p) => Facing::ALL
                .into_iter()
                .any(|f| p.depth_only.is_built(f) && std::ptr::eq(p.depth_only.get(f), pipeline)),
            _ => false,
        };
        PLAIN.with(|c| c.set(plain));
    }

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

    pub(crate) fn take() -> PrepassDraws {
        COUNTS.with(Cell::take)
    }
}
