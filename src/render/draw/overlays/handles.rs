//! The editor's handles and gizmos (#1224): the floor grid, the occlusion view's
//! boxes, the selection's axis arrows and the probe gizmos (as the Scene view's set
//! has them, #1909) and the colliders and debug lines (#1882, the only ones a game
//! frame with its gizmos on draws), drawn the way Unity draws them —
//! after post-FX and the selection outline, before the UI. On the finished image no
//! grade, bloom, SSR, motion blur or FXAA treats them as scene colour, and the
//! outline's ring never paints over an arrow shaft. They are still depth-tested
//! against the scene's depth, which post-FX only reads.

use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// Draw `scene`'s editor gizmos onto `output`, the view's finished post-FX image,
    /// tested against the view's depth from the camera that just drew (the stack's
    /// last, the editor's one camera). Group 0 still holds that camera's uniform.
    /// `eye` is that camera's position; with `editor` false this is a game frame's
    /// gizmos (#1882): only the colliders and the debug lines.
    pub(crate) fn draw_handles(
        &self,
        view: &RenderView,
        scene: &Scene,
        output: &wgpu::TextureView,
        (eye, editor): (glam::Vec3, bool),
    ) {
        let overlays = match editor {
            true => self.precreate_overlays(scene, &view.gizmos, eye),
            false => self.precreate_game_overlays(scene, eye),
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Editor Handles Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Editor Handles Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    // The lines test the scene's depth but never write it.
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.gpu_timer.writes(GpuPass::Outline),
                occlusion_query_set: None,
            });
            pass.set_pipeline(self.line_pipeline.get());
            pass.set_bind_group(0, &self.global_bind_group, &[]);
            // The lines never sample a material; the grid rebinds this same group.
            pass.set_bind_group(2, &self.default_material_bind_group, &[]);
            // Group 3 is in the forward layout; unlit lines read nothing from it.
            pass.set_bind_group(3, &self.shadow_bind_group, &[]);
            self.draw_editor_overlays(&mut pass, &overlays);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
