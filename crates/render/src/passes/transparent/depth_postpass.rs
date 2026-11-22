use crate::draw::batch::DrawBatch;
use crate::gpu::groups;
use crate::gpu::ops::CountedEncoder as _;
use crate::gpu::pipelines::surface::SolidPass;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

pub(crate) fn writers(translucents: &[DrawBatch]) -> impl Iterator<Item = &DrawBatch> {
    translucents.iter().filter(|b| b.key.traits.depth_postpass)
}

pub(crate) fn movers(translucents: &[DrawBatch]) -> impl Iterator<Item = &DrawBatch> {
    let t = |b: &&DrawBatch| b.key.traits;
    translucents
        .iter()
        .filter(move |b| t(b).writes_motion && !t(b).depth_postpass)
}

fn targets(
    velocity: Option<&wgpu::TextureView>,
) -> Vec<Option<wgpu::RenderPassColorAttachment<'_>>> {
    let load = |view| {
        Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })
    };
    match velocity {
        Some(v) => vec![None, load(v)],
        None => Vec::new(),
    }
}

impl Renderer {
    pub(crate) fn draw_depth_postpass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        translucents: &[DrawBatch],
    ) {
        let velocity = view.post_fx.motion.target_view();
        let depth: Vec<DrawBatch> = writers(translucents).cloned().collect();
        let motion: Vec<DrawBatch> = match velocity {
            Some(_) => movers(translucents).cloned().collect(),
            None => Vec::new(),
        };
        if depth.is_empty() && motion.is_empty() {
            return;
        }
        {
            let targets = targets(velocity);
            let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Depth Postpass"),
                color_attachments: &targets,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.gpu_timer.writes(GpuPass::Transparent),
                occlusion_query_set: None,
            });
            pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
            pass.set_bind_group(groups::SCENE, &self.shadow_bind_group, &[]);
            let pipelines = &self.depth_postpass_pipelines;
            let solid_pass = SolidPass::DepthPostpass(pipelines, velocity.is_some());
            self.draw_batches(&mut pass, &depth, solid_pass);
            self.draw_batches(&mut pass, &motion, SolidPass::MotionPostpass(pipelines));
        }
    }
}
