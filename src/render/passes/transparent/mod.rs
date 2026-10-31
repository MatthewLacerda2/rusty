//! src/render/passes/transparent/mod.rs — the alpha-blended transparent solids pass (#242).
//!
//! Drawn after opaque solids + decals, before particles/post-FX. The draw list is the
//! Transparent-material entities for one camera, already sorted back-to-front by the
//! `precreate_solid_resources` split, so `ALPHA_BLENDING` composites correctly (nearer
//! glass over farther glass over the opaque world).
//!
//! It reuses the camera's instanced draw data the opaque pass binds (group 1 frame
//! buffers, group 2 shared material, #470) and the same shader; only the pipeline differs
//! (`transparent_pipelines`: one per blend state, #1523; depth test on / write off). It LOADs the
//! existing HDR colour + depth — never clears — so it layers onto the opaque image and
//! is occluded by opaque depth without writing depth itself (overlapping translucent
//! surfaces all blend rather than z-cull each other).

mod depth_postpass;
pub(crate) mod opaque_copy;

pub(crate) use opaque_copy::{OpaqueCopyPass, OpaqueViews};

use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

impl Renderer {
    /// Draw the camera's translucent solids into `view`'s HDR target. `items` is already
    /// back-to-front (farthest first); only neighbouring copies were merged (#470). No-op when there is nothing translucent,
    /// so opaque-only scenes pay nothing (#242).
    pub(crate) fn draw_transparent(&self, view: &RenderView, items: &[DrawBatch]) {
        if items.is_empty() {
            return;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Transparent Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Transparent Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &view.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    // Load the opaque depth so glass is occluded by solids in front,
                    // but the transparent pipeline writes no depth (store keeps it as-is
                    // for any later pass that reads it).
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.gpu_timer.writes(GpuPass::Transparent),
                occlusion_query_set: None,
            });
            self.record_transparent(&mut pass, self.transparent_group3(view), items);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Record the back-to-front transparent draws into `pass`: the global bind group
    /// and `group3` (the shadows, and the opaque copy when this camera made one,
    /// #1523), then the same batch recording the opaque pass uses — each batch
    /// binding the transparent pipeline of its material's shader (#396) and blend.
    fn record_transparent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        group3: &'a wgpu::BindGroup,
        items: &'a [DrawBatch],
    ) {
        pass.set_bind_group(0, &self.global_bind_group, &[]);
        pass.set_bind_group(3, group3, &[]);
        self.draw_batches(
            pass,
            items,
            SolidPass::Transparent(&self.transparent_pipelines),
        );
    }
}

#[cfg(test)]
#[path = "opaque_copy_gpu_tests.rs"]
mod opaque_copy_gpu_tests;
