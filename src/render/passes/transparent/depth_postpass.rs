//! The transparent depth postpass (#1523), HDRP's *Transparent Depth Postpass*: after
//! a camera's translucents drew, the ones whose material asks for it
//! (`depth_postpass`) draw again into the view's depth, and into its velocity
//! target when it has one, so the post effects that read depth (depth of field,
//! motion blur) and the passes after it (soft particles, world canvases) see the
//! surface rather than what is behind it. Fog needs none: the forward shader fogs
//! each surface itself. SSR runs before the translucents (HDRP's order), so it never
//! reflects on them.

use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

/// The batches of `translucents` that write their depth after drawing.
pub(crate) fn writers(translucents: &[DrawBatch]) -> impl Iterator<Item = &DrawBatch> {
    translucents
        .iter()
        .filter(|b| b.key.record().surface.depth_postpass == 1)
}

impl Renderer {
    /// Draw the depth postpass of `translucents` into `view`; nothing when none asks.
    pub(crate) fn draw_depth_postpass(&self, view: &RenderView, translucents: &[DrawBatch]) {
        let batches: Vec<DrawBatch> = writers(translucents).map(DrawBatch::clone).collect();
        if batches.is_empty() {
            return;
        }
        let velocity = view.post_fx.motion.target_view();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Depth Postpass Encoder"),
            });
        {
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
            let targets = match velocity {
                Some(v) => vec![None, load(v)],
                None => Vec::new(),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            pass.set_bind_group(0, &self.global_bind_group, &[]);
            pass.set_bind_group(3, &self.shadow_bind_group, &[]);
            let pipelines = &self.depth_postpass_pipelines;
            let solid_pass = SolidPass::DepthPostpass(pipelines, velocity.is_some());
            self.draw_batches(&mut pass, &batches, solid_pass);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
