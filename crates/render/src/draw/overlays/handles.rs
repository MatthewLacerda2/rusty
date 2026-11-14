//! The editor's handles and gizmos (#1224): the floor grid, the occlusion view's
//! boxes, the selection's axis arrows and the probe gizmos (as the Scene view's set
//! has them, #1909) and the colliders and debug lines (#1882), drawn the way Unity
//! draws them — after post-FX and the selection outline, before the UI. On the
//! finished image no grade, bloom, SSR, motion blur or FXAA treats them as scene
//! colour, and the outline's ring never paints over an arrow shaft. They are still
//! depth-tested against the scene's depth, which post-FX only reads. A game
//! frame's gizmos (only the colliders and debug lines) draw through the same pass
//! (`game`).

use crate::draw::resources::Overlays;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};
use rusty_scene::Scene;

impl Renderer {
    /// Draw `scene`'s editor gizmos onto `output`, the view's finished post-FX image,
    /// tested against the view's depth from the camera that just drew (the stack's
    /// last, the editor's one camera, at `eye`). Group 0 still holds its uniform.
    pub(crate) fn draw_handles(
        &self,
        view: &RenderView,
        scene: &Scene,
        output: &wgpu::TextureView,
        eye: glam::Vec3,
    ) {
        let overlays = self.precreate_overlays(scene, &view.gizmos, eye);
        self.draw_overlay_pass(&overlays, output, &view.depth_view);
    }

    /// Draw `overlays` onto `output`, tested against `depth` (never written), with
    /// the camera uniform group 0 holds now.
    pub(super) fn draw_overlay_pass(
        &self,
        overlays: &Overlays,
        output: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
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
                    view: depth,
                    // The lines test the scene's depth but never write it; the probe
                    // balls (#1911) write it, and draw first so a line hides behind one.
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
            self.draw_editor_overlays(&mut pass, overlays);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
