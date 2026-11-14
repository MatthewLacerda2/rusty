//! The transparent depth postpass (#1523), HDRP's *Transparent Depth Postpass*: after
//! a camera's translucents drew, the ones whose material asks for it
//! (`depth_postpass`) draw again into the view's depth, and into its velocity
//! target when it has one, so the post effects that read depth (depth of field,
//! motion blur) and the passes after it (soft particles, world canvases) see the
//! surface rather than what is behind it. Fog needs none: the forward shader fogs
//! each surface itself. SSR runs before the translucents (HDRP's order), so it never
//! reflects on them.
//!
//! Then the ones that write their motion alone (`writes_motion` without
//! `depth_postpass`, #1845), HDRP's *Transparent Writes Motion Vectors*: into the
//! velocity target only (their motion, view depth and camera), depth tested against
//! what the depth writers left but not written, so TAA, motion blur and depth of
//! field move them as solids. A view with no velocity target draws none of them.

use crate::draw::batch::DrawBatch;
use crate::gpu::pipelines::surface::SolidPass;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

/// The batches of `translucents` that write their depth after drawing.
pub(crate) fn writers(translucents: &[DrawBatch]) -> impl Iterator<Item = &DrawBatch> {
    translucents.iter().filter(|b| b.key.traits.depth_postpass)
}

/// The batches of `translucents` that write their motion but not their depth.
pub(crate) fn movers(translucents: &[DrawBatch]) -> impl Iterator<Item = &DrawBatch> {
    let t = |b: &&DrawBatch| b.key.traits;
    translucents
        .iter()
        .filter(move |b| t(b).writes_motion && !t(b).depth_postpass)
}

/// The pass's colour attachments: none at location 0, and the `velocity` target,
/// loaded, at location 1 when the view has one.
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
    /// Draw the depth postpass of `translucents` into `view`, then the motion
    /// writers'; nothing when none asks.
    pub(crate) fn draw_depth_postpass(&self, view: &RenderView, translucents: &[DrawBatch]) {
        let velocity = view.post_fx.motion.target_view();
        let depth: Vec<DrawBatch> = writers(translucents).cloned().collect();
        let motion: Vec<DrawBatch> = match velocity {
            Some(_) => movers(translucents).cloned().collect(),
            None => Vec::new(),
        };
        if depth.is_empty() && motion.is_empty() {
            return;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Depth Postpass Encoder"),
            });
        {
            let targets = targets(velocity);
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
            self.draw_batches(&mut pass, &depth, solid_pass);
            self.draw_batches(&mut pass, &motion, SolidPass::MotionPostpass(pipelines));
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
