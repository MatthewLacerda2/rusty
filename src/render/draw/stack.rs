//! src/render/draw/stack.rs — one camera stack drawn into one view: each camera runs
//! the per-camera pass list (`camera::CAMERA_PASSES`, #636), whose last stage
//! post-processes the composited HDR target once, on the last camera. The screen stack and every render-texture camera (#430)
//! draw through here, so a picture-in-picture is the same frame as the screen's.

use super::camera::{CameraCtx, StackShared, CAMERA_PASSES};
use crate::render::gpu::uniforms::FogUniform;
use crate::render::passes::ssao::SsaoPlan;
use crate::render::postfx::params::build_post_params;
use crate::render::postfx::PostParams;
use crate::render::{RenderView, Renderer};
use crate::scene::{Camera, Scene};

/// What one [`Renderer::draw_stack`] call draws.
pub(crate) struct StackPass<'a> {
    /// The cameras, bottom to top; the first clears and drives post-FX history.
    pub stack: &'a [Camera],
    /// Where post-FX writes the finished image.
    pub output: &'a wgpu::TextureView,
    pub editor_mode: bool,
    /// Run the scene's grading, bloom, SSR, motion blur and FXAA. Off, the image is
    /// only brought from HDR to display (a render texture's `post_fx = false`).
    pub full_post_fx: bool,
}

impl Renderer {
    /// Draw `pass.stack` into `view` and post-process it to `pass.output`. Shadows and
    /// the frame's shared uploads are the caller's (see `Renderer::render`). Each
    /// camera runs the [`CAMERA_PASSES`] table in order.
    pub(crate) fn draw_stack(&mut self, view: &mut RenderView, scene: &Scene, pass: StackPass) {
        let aspect = view.aspect();
        let shared = StackShared {
            aspect,
            // The base (first) camera drives the shared post-FX history / motion vectors.
            base_view_proj: pass.stack[0].build_view_projection(aspect),
            fog: FogUniform::from_settings(&scene.fog),
            ssao: SsaoPlan::for_scene(scene, self.quality),
        };
        for idx in 0..pass.stack.len() {
            let mut ctx = CameraCtx::new(scene, &pass, &shared, idx);
            for (_, stage) in CAMERA_PASSES {
                stage(self, view, &mut ctx);
            }
        }
    }

    /// Run the post-process chain (color correction, bloom, motion blur, SSR) over the
    /// view's composited HDR target, writing the corrected image to `output`.
    pub(super) fn run_post_fx(&mut self, view: &mut RenderView, ctx: &CameraCtx) {
        use crate::render::postfx::PostFxContext;

        let (scene, output) = (ctx.scene, ctx.stack.output);
        let post = PostFrame {
            view_proj: ctx.base_view_proj,
            camera_pos: ctx.cam.position.to_array(),
            full: ctx.stack.full_post_fx,
        };

        let (mut post_params, bloom_enabled) = build_post_params(
            scene,
            self.quality,
            post.view_proj,
            view.post_fx.prev_view_proj,
            post.camera_pos,
        );
        let mut passes = crate::render::postfx::params::post_passes(scene, bloom_enabled);
        if !post.full {
            // Display transform only: the neutral grade, no bloom/SSR/blur/FXAA and
            // no authored effects.
            let neutral = PostParams::default();
            post_params.color = neutral.color;
            post_params.bloom = neutral.bloom;
            post_params.flags = neutral.flags;
            post_params.misc[3] = neutral.misc[3];
            (passes.bloom, passes.fxaa) = (false, false);
            passes.custom.clear();
        }
        post_params.misc[1] = 1.0 / view.post_fx.bloom_size.0 as f32;
        post_params.misc[2] = 1.0 / view.post_fx.bloom_size.1 as f32;
        view.post_fx.prev_view_proj = post.view_proj;

        let skybox_view = self
            .skybox_texture
            .as_ref()
            .map(|tex| &tex.view)
            .unwrap_or(&self.default_texture.view);
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox_view,
            output,
        };
        view.post_fx
            .run(&self.device, &self.queue, ctx, post_params, passes);
    }
}

/// The per-stack inputs of [`Renderer::run_post_fx`].
struct PostFrame {
    view_proj: glam::Mat4,
    camera_pos: [f32; 3],
    full: bool,
}
