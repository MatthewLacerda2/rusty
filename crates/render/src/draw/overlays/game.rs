//! Gizmos use the base camera, as Unity's Game view does: in a stack the last camera is the
//! viewmodel, whose fov, near plane and cleared depth would misplace them.

use crate::draw::camera::buffer::Slot;
use crate::draw::camera::CameraCtx;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::effects::taa;
use crate::{RenderView, Renderer};

impl Renderer {
    pub(crate) fn keep_gizmo_depth(&self, view: &mut RenderView, ctx: &mut CameraCtx) {
        let size = view.depth_texture.size();
        let stale = view
            .gizmo_depth
            .as_ref()
            .is_none_or(|(kept, _)| kept.size() != size);
        if stale {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Game Gizmo Depth"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let depth = texture.create_view(&wgpu::TextureViewDescriptor::default());
            view.gizmo_depth = Some((texture, depth));
        }
        let Some((kept, _)) = &view.gizmo_depth else {
            return;
        };
        let from = view.depth_texture.as_image_copy();
        let encoder = ctx.commands.encoder();
        encoder.copy_texture_to_texture(from, kept.as_image_copy(), size);
    }

    pub(crate) fn gizmos_upscaled(&self, view: &RenderView, ctx: &CameraCtx) -> bool {
        let stack = ctx.stack;
        let editor = stack.editor_mode;
        let graphics = self.graphics(editor);
        let taa_on = taa::runs(view, stack.post, graphics, stack.full_post_fx, editor);
        taa::upscales(view, taa_on)
    }

    pub(crate) fn draw_game_gizmos(&self, view: &mut RenderView, ctx: &mut CameraCtx) {
        let stack = ctx.stack;
        if self.gizmos_upscaled(view, ctx) {
            if !view.gizmo_upscale_told {
                view.gizmo_upscale_told = true;
                log::warn!(
                    "[Render] Gizmos: not drawn while TAA upscales the view (render scale below 1); set it to 1 to see them"
                );
            }
            return;
        }
        let depth = match ctx.is_first {
            true => &view.depth_view,
            false => match &view.gizmo_depth {
                Some((_, kept)) => kept,
                None => return,
            },
        };
        let base = &stack.stack[0];
        let mut uniform = ctx.uniform;
        uniform.view_proj = ctx.base_view_proj.to_cols_array();
        uniform.inv_view_proj = ctx.base_view_proj.inverse().to_cols_array();
        uniform.camera_pos = base.position.to_array();
        let (camera, queue) = (&self.camera_buffer, &self.queue);
        camera.record((queue, ctx.commands.encoder()), Slot::GizmoBase, &uniform);
        let overlays = self.precreate_game_overlays(ctx.scene, base.position);
        self.draw_overlay_pass(ctx.commands.encoder(), &overlays, stack.output, depth);
        camera.record(
            (queue, ctx.commands.encoder()),
            Slot::GizmoBack,
            &ctx.uniform,
        );
    }
}

#[cfg(test)]
#[path = "game_gpu_tests.rs"]
mod gpu_tests;
