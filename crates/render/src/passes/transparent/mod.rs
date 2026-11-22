mod depth_postpass;
pub(crate) mod opaque_copy;

use crate::gpu::groups;
use crate::gpu::ops::CountedEncoder as _;
pub(crate) use opaque_copy::{OpaqueCopyPass, OpaqueViews};

use crate::draw::batch::DrawBatch;
use crate::gpu::pipelines::surface::SolidPass;
use crate::postfx::effects::taa;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

impl Renderer {
    pub(crate) fn draw_transparent(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        items: &[DrawBatch],
    ) {
        if items.is_empty() {
            return;
        }
        let reactive = view.post_fx.taa_reactive();
        let targets = taa::reactive::hdr_attachments(view);
        {
            let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Transparent Pass"),
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
            let scene = self.transparent_scene_group(view);
            self.record_transparent(&mut pass, scene, items, reactive.is_some());
        }
    }

    fn record_transparent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        scene: &'a wgpu::BindGroup,
        items: &'a [DrawBatch],
        reactive: bool,
    ) {
        pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
        pass.set_bind_group(groups::SCENE, scene, &[]);
        self.draw_batches(
            pass,
            items,
            SolidPass::Transparent(&self.transparent_pipelines, reactive),
        );
    }
}

#[cfg(test)]
#[path = "opaque_copy_gpu_tests.rs"]
mod opaque_copy_gpu_tests;
