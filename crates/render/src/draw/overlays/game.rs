//! A game frame's gizmos (#1882): the colliders and the `Debug.Draw*` lines over
//! the finished image, as Unity's Game view draws them with its **Gizmos** toggle
//! on (`RenderView::game_gizmos`).
//!
//! **Through the base camera.** Unity's Game-view gizmos follow the main camera. In
//! a stack (a world camera under a `DepthOnly` viewmodel) the last camera, whose
//! uniform group 0 holds when the handles stage runs, is the viewmodel: its field of
//! view and near plane would misplace every outline, and its cleared depth would
//! let them show through walls. So the gizmos draw with the base camera's
//! (unjittered) view-projection, against its depth: kept by the `gizmo_depth`
//! stage before the next camera clears it, or the view's own when the base camera
//! is the only one.
//!
//! **Not under TAA upscaling.** There the finished image is the output's size and
//! the depth the scene's, which no pass can attach together; the gizmos are left
//! out, and the console says so once per view. Their depth is not kept there, and
//! the kept texture is freed whenever nothing will read it (gizmos off, or
//! upscaling).

use crate::draw::camera::CameraCtx;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::effects::taa;
use crate::{RenderView, Renderer};

impl Renderer {
    /// Keep the base camera's depth for the game gizmos, before a stacked camera
    /// clears it.
    pub(crate) fn keep_gizmo_depth(&self, view: &mut RenderView) {
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
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Game Gizmo Depth Copy"),
            });
        let from = view.depth_texture.as_image_copy();
        encoder.copy_texture_to_texture(from, kept.as_image_copy(), size);
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Whether TAA upscales this stack's view, where the game gizmos are not drawn
    /// (the module docs), so their depth is not kept either.
    pub(crate) fn gizmos_upscaled(&self, view: &RenderView, ctx: &CameraCtx) -> bool {
        let stack = ctx.stack;
        let editor = stack.editor_mode;
        let graphics = self.graphics(editor);
        let taa_on = taa::runs(view, stack.post, graphics, stack.full_post_fx, editor);
        taa::upscales(view, taa_on)
    }

    /// Draw the game gizmos onto the stack's output, on its last camera.
    pub(crate) fn draw_game_gizmos(&self, view: &mut RenderView, ctx: &CameraCtx) {
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
        let camera = &self.camera_buffer;
        self.queue
            .write_buffer(camera, 0, bytemuck::bytes_of(&uniform));
        let overlays = self.precreate_game_overlays(ctx.scene, base.position);
        self.draw_overlay_pass(&overlays, stack.output, depth);
        // Back to this camera's own, for anything after the stack that reads it.
        self.queue
            .write_buffer(camera, 0, bytemuck::bytes_of(&ctx.uniform));
    }
}

#[cfg(test)]
#[path = "game_gpu_tests.rs"]
mod gpu_tests;
