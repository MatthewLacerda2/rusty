//! src/render/draw/stack.rs — one camera stack drawn into one view: each camera runs
//! the per-camera pass list (`camera::CAMERA_PASSES`, #636), whose last stage
//! post-processes the composited HDR target once, on the last camera, with the stack's
//! base camera's settings (#1434). The screen stack and every render-texture camera (#430)
//! draw through here, so a picture-in-picture is the same frame as the screen's.

use super::camera::{CameraCtx, StackShared, CAMERA_PASSES};
use crate::render::gpu::uniforms::FogUniform;
use crate::render::motion::{self, HistoryKey};
use crate::render::passes::ssao::SsaoPlan;
use crate::render::postfx::grade::GradeParams;
use crate::render::postfx::params::{blur_params, build_post_params, post_passes, PostSource};
use crate::render::postfx::PostParams;
use crate::render::{RenderView, Renderer};
use crate::scene::{Camera, Scene};

/// What one [`Renderer::draw_stack`] call draws.
pub(crate) struct StackPass<'a> {
    /// The cameras, bottom to top; the first clears and drives post-FX history.
    pub stack: &'a [Camera],
    /// Each camera's entity, beside `stack` (`None`: the editor's free camera); a
    /// camera's motion-vector history is kept by it (#1435).
    pub entities: &'a [Option<u32>],
    /// The base camera's entity, `scene::camera::main_camera` in either mode (`None`
    /// when the scene has no screen camera); another one than last frame's is a
    /// camera cut (#1435).
    pub base_id: Option<u32>,
    /// The camera and volume settings that drive this view's post-FX and SSAO: the
    /// stack's base camera's (#1434, Unity URP's rule), borrowed once per view.
    pub post: PostSource<'a>,
    /// Where post-FX writes the finished image.
    pub output: &'a wgpu::TextureView,
    pub editor_mode: bool,
    /// Run `post`'s grading, bloom, SSR, motion blur and FXAA. Off, the image is
    /// only brought from HDR to display (a render texture's `post_fx = false`).
    pub full_post_fx: bool,
    /// How many frames of motion this draw's velocities span: 1 for the screen, a
    /// render texture's `update_every` (#1436), so its blur is one frame's shutter.
    pub motion_frames: u32,
}

impl Renderer {
    /// Draw `pass.stack` into `view` and post-process it to `pass.output`. Shadows and
    /// the frame's shared uploads are the caller's (see `Renderer::render`). Each
    /// camera runs the [`CAMERA_PASSES`] table in order.
    pub(crate) fn draw_stack(&mut self, view: &mut RenderView, scene: &Scene, pass: StackPass) {
        let aspect = view.aspect();
        let view_projs: Vec<_> = pass
            .stack
            .iter()
            .map(|c| c.build_view_projection(aspect))
            .collect();
        let cameras: Vec<_> = pass
            .entities
            .iter()
            .copied()
            .zip(view_projs.iter().copied())
            .collect();
        // Motion vectors (#1435): last frame's cameras, and the velocity target when
        // this view runs motion blur. A view shading with its own forward pipeline
        // (the Inspector's shader preview) writes none.
        let velocity = motion::wanted(
            pass.post,
            self.graphics(pass.editor_mode),
            pass.full_post_fx,
            pass.editor_mode,
        ) && !view.has_forward_override();
        let key = HistoryKey::new(scene, pass.base_id);
        let size = view.size();
        let prev_view_projs =
            view.post_fx
                .motion
                .begin(&self.device, size, key, velocity, &cameras);
        let shared = StackShared {
            aspect,
            view_projs,
            prev_view_projs,
            fog: FogUniform::from_settings(&scene.fog),
            ssao: SsaoPlan::for_volume(pass.post.volume, self.graphics(pass.editor_mode)),
        };
        for idx in 0..pass.stack.len() {
            let mut ctx = CameraCtx::new(scene, &pass, &shared, idx);
            for (_, stage) in CAMERA_PASSES {
                stage(self, view, &mut ctx);
            }
        }
        view.post_fx.motion.history.end();
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
        let graphics = self.graphics(ctx.stack.editor_mode);

        // Per-object motion blur (#1436), over the velocity target the stack wrote:
        // bloom and the composite below then read the blurred scene. Gated as the
        // velocity was, so the editor's Scene view never blurs.
        let stack = ctx.stack;
        let blurs = motion::wanted(stack.post, graphics, stack.full_post_fx, stack.editor_mode);
        let blur = blur_params(stack.post, graphics, view.size().height)
            .filter(|_| blurs)
            .map(|b| b.per_frames(stack.motion_frames));
        let gpu = (&self.device, &self.queue);
        view.post_fx
            .run_motion_blur(gpu, &self.motion_blur, blur, &self.gpu_timer);

        // The grade + tonemap (#1440): rebaked only when this view's inputs changed.
        let grade = match post.full {
            true => GradeParams::from_volume(ctx.stack.post.volume),
            false => GradeParams::NEUTRAL,
        };
        let gpu = (&self.device, &self.queue);
        let baked = view.post_fx.grade.prepare(gpu, &self.grade_bake, &grade);
        self.frame_counters.grade_lut_bakes += u32::from(baked);

        let (mut post_params, bloom_enabled) = build_post_params(
            scene,
            ctx.stack.post,
            graphics,
            post.view_proj,
            post.camera_pos,
        );
        let mut passes = post_passes(ctx.stack.post, bloom_enabled, graphics);
        if !post.full {
            // Display transform only: the neutral grade, no bloom/SSR/blur/FXAA and
            // no authored effects.
            let neutral = PostParams::default();
            post_params.color = neutral.color;
            post_params.bloom = neutral.bloom;
            post_params.flags = neutral.flags;
            (passes.bloom, passes.fxaa) = (false, false);
            passes.custom = &[];
        }
        post_params.misc[1] = 1.0 / view.post_fx.bloom_size.0 as f32;
        post_params.misc[2] = 1.0 / view.post_fx.bloom_size.1 as f32;

        let sky = self.sky_panorama();
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox: (&sky.view, &sky.sampler),
            output,
            timer: &self.gpu_timer,
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
