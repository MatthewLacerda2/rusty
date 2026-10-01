//! src/render/draw/stack.rs — one camera stack drawn into one view: each camera's
//! solids, decals, transparents, world UI and particles, then post-FX once over the
//! composited HDR target. The screen stack and every render-texture camera (#430)
//! draw through here, so a picture-in-picture is the same frame as the screen's.

use super::pass::{PassClear, ScenePassFrame};
use crate::render::gpu::uniforms::FogUniform;
use crate::render::lod::LodSelection;
use crate::render::passes::ssao::{SsaoFrame, SsaoPlan};
use crate::render::postfx::params::build_post_params;
use crate::render::postfx::PostParams;
use crate::render::{CameraUniform, RenderView, Renderer};
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
    /// the frame's shared uploads are the caller's (see `Renderer::render`).
    pub(crate) fn draw_stack(&mut self, view: &mut RenderView, scene: &Scene, pass: StackPass) {
        let aspect = view.aspect();
        let last = pass.stack.len().saturating_sub(1);
        // The base (first) camera drives the shared post-FX history / motion vectors.
        let base_view_proj = pass.stack[0].build_view_projection(aspect);
        let ssao = SsaoPlan::for_scene(scene, self.quality);
        let fog = FogUniform::from_settings(&scene.fog);

        for (idx, cam) in pass.stack.iter().enumerate() {
            // 1. Write this camera's view/projection uniform.
            let view_proj = cam.build_view_projection(aspect);
            // This camera's world-space frustum, for culling off-screen entities (#330).
            let frustum = crate::render::Frustum::from_view_proj(view_proj);
            let camera_uniform = CameraUniform {
                view_proj: view_proj.to_cols_array(),
                camera_pos: cam.position.to_array(),
                time: scene.shader_time,
                fog,
            };
            self.queue
                .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

            // 2. Batch this camera's solids (the culling mask differs per camera) into
            // instanced draws (#470), split opaque/cutout vs transparent (#242). Each
            // camera picks its own LOD levels (#472) before batching.
            let lod = LodSelection::for_camera(scene, cam);
            let solids = self.precreate_solid_resources(scene, cam, &frustum, &lod);
            let overlays = self.precreate_overlays(scene, pass.editor_mode);

            let frame = ScenePassFrame {
                editor_mode: pass.editor_mode,
                clear: PassClear::for_pass(idx == 0, cam.clear_flags),
                ssao: ssao.map(|plan| SsaoFrame::for_camera(plan, view_proj, cam)),
            };
            self.execute_scene_pass(view, frame, &solids.draws.opaque, &overlays);

            // Decals over this camera's lit surfaces, before the transparents and
            // particles so they sit on the surface, not over the sparks.
            self.draw_decals(view, scene, cam);
            // Translucent solids (#242), back-to-front over opaque + decals.
            self.draw_transparent(view, &solids.draws.transparent);
            // World canvases (#429): scene geometry, occluded by it, before particles.
            self.draw_world_ui(view, cam, aspect, fog);

            // Ribbons (#441) then sprite particles for this camera, each back to front;
            // mesh particles drew with the solids.
            let effects = self.draw_effects(view, scene, cam);
            self.count_camera(&solids, scene.decals.len(), effects);

            // 3. Composite + post-process once, over the final pass's HDR target.
            if idx == last {
                let post = PostFrame {
                    view_proj: base_view_proj,
                    camera_pos: cam.position.to_array(),
                    full: pass.full_post_fx,
                };
                self.run_post_fx(view, scene, pass.output, post);
            }
        }
    }

    /// Run the post-process chain (color correction, bloom, motion blur, SSR) over the
    /// view's composited HDR target, writing the corrected image to `output`.
    fn run_post_fx(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        output: &wgpu::TextureView,
        post: PostFrame,
    ) {
        use crate::render::postfx::PostFxContext;

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
