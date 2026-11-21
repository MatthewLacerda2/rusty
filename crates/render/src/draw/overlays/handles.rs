use crate::draw::resources::Overlays;
use crate::gpu::groups;
use crate::gpu::ops::CountedEncoder as _;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};
use rusty_scene::Scene;

impl Renderer {
    pub(crate) fn draw_handles(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        scene: &Scene,
        output: &wgpu::TextureView,
        eye: glam::Vec3,
    ) {
        let overlays = self.precreate_overlays(scene, &view.gizmos, eye);
        self.draw_overlay_pass(encoder, &overlays, output, &view.depth_view);
    }

    pub(super) fn draw_overlay_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        overlays: &Overlays,
        output: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        {
            let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
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
            pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
            pass.set_bind_group(groups::MATERIAL, &self.default_material_bind_group, &[]);
            pass.set_bind_group(groups::SCENE, &self.shadow_bind_group, &[]);
            self.draw_editor_overlays(&mut pass, overlays);
        }
    }
}
