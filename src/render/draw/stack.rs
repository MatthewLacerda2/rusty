//! src/render/draw/stack.rs — one camera stack drawn into one view: each camera runs
//! the per-camera pass list (`camera::CAMERA_PASSES`, #636), whose last stage
//! post-processes the composited HDR target once, on the last camera, with the stack's
//! base camera's settings (#1434). The screen stack and every render-texture camera (#430)
//! draw through here, so a picture-in-picture is the same frame as the screen's.

use super::camera::{CameraCtx, StackShared, CAMERA_PASSES};
use crate::core::settings::GraphicsSettings;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::motion::{self, HistoryKey, Velocity};
use crate::render::passes::ssao::SsaoPlan;
use crate::render::postfx::effects::ssil;
use crate::render::postfx::effects::ssr::{ssr_enabled, surface, transparent};
use crate::render::postfx::effects::{self, taa, volumetric_fog, EffectCx, PassCx};
use crate::render::postfx::params::{frame_params, PostSource};
use crate::render::{RenderView, Renderer};
use crate::scene::{Camera, Scene};

/// Whether this frame of `view` reads what an earlier one left (#2092), so drawing
/// it again with no sim step can change the image: an effect that keeps a history
/// runs ([`effects::History`]), or motion vectors compare with last frame's poses
/// on a frame that is not a cut. (A shown render texture and a pending shadow tile
/// resize are counted where they are drawn.)
fn reads_history(
    view: &RenderView,
    (pass, graphics, taa): (&StackPass, GraphicsSettings, bool),
    shared: &StackShared,
) -> bool {
    let effect = EffectCx {
        post: pass.post,
        graphics,
        full: pass.full_post_fx,
    };
    let motion = &view.post_fx.motion;
    let moving = motion.active() && !motion.history.cut();
    moving
        || effects::read_history(&effects::HistoryCx {
            effect,
            shared,
            taa,
        })
}

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
    /// Where post-FX writes the finished image, and the editor's overlays draw: a
    /// scaled view's scene-size target ([`RenderView::scaled_output`]) on the
    /// bilinear path, else the output itself (a native view, or one TAA upscales,
    /// TAAU #1702, whose chain runs at the output's size).
    pub output: &'a wgpu::TextureView,
    pub editor_mode: bool,
    /// Run `post`'s grading, bloom, SSR, TAA, motion blur and FXAA. Off, the image is
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
        let (view_projs, cameras) = matrices(&pass, aspect);
        // Motion vectors (#1435): last frame's cameras, and the velocity target when
        // this view runs motion blur, TAA (#1702), depth of field (#1445) or SSR, which reads its
        // depths beside its own surface targets (#1526). A view shading
        // with its own forward pipeline (the Inspector's shader preview) writes none.
        let graphics = self.graphics(pass.editor_mode);
        let own_pipeline = view.has_forward_override();
        let ssr = ssr_enabled(pass.post, graphics, pass.full_post_fx) && !own_pipeline;
        let moving =
            motion::target_wanted(pass.post, graphics, pass.full_post_fx, pass.editor_mode)
                && !own_pipeline;
        let velocity = velocity_target(moving, ssr);
        // TAA (#1702): whether it runs, so the chain's size, before any effect fits.
        let taa_on = taa::runs(
            view,
            pass.post,
            graphics,
            pass.full_post_fx,
            pass.editor_mode,
        );
        view.post_fx
            .fit_chain(&self.device, taa::chain_size(view, taa_on));
        let key = HistoryKey::new(scene, pass.base_id);
        let size = view.scene_size();
        surface::begin(&mut view.post_fx.targets, &self.device, size, ssr);
        let prev_view_projs =
            view.post_fx
                .motion
                .begin(&self.device, size, key, velocity, &cameras);
        let prev_time = view.post_fx.motion.history.game_time(scene.shader_time);
        // SSIL (#1750) and volumetric fog (#1703) keep a history per view: a bake
        // capture's faces and the shader preview keep none, so they never run them.
        let keeps = !own_pipeline && !self.capture.static_only;
        // TAA (#1702): the frame's jitter and the maps' mip bias, tuned by the base
        // camera's knobs (#1847); a cut forgets its history.
        let taa = taa::begin(view, &self.device, taa_on, taa_knobs(pass.post.camera));
        let shared = StackShared {
            aspect,
            view_projs,
            prev_view_projs,
            prev_time,
            taa,
            fog: FogUniform::from_settings(&scene.fog),
            ssao: SsaoPlan::for_volume(pass.post.volume, self.graphics(pass.editor_mode)),
            ssr_march: transparent::camera_march(pass.post, graphics, ssr),
            ssil: ssil::begin(&mut view.post_fx, (&pass, graphics, key), keeps),
            volumetric: volumetric_fog::begin(&mut view.post_fx, (&pass, graphics, key), keeps),
        };
        let history = reads_history(view, (&pass, graphics, taa_on), &shared);
        self.frame_counters.history_passes += u32::from(history);
        for idx in 0..pass.stack.len() {
            let mut ctx = CameraCtx::new(scene, &pass, &shared, idx);
            for (_, stage) in CAMERA_PASSES {
                stage(self, view, &mut ctx);
            }
        }
        view.post_fx.motion.history.end();
    }

    /// Run the post-process chain over the view's composited HDR target: every
    /// effect's scene pass in order (depth of field, TAA, motion blur, the exposure
    /// meter, bloom, the lens flare), the grading LUT's bake when its inputs changed,
    /// then the composite and the LDR passes, writing the corrected image to
    /// `StackPass::output`. When TAA upscales the view (TAAU, #1702), everything
    /// after it runs at the output's size and `output` is the real output, so the
    /// caller has no scaled target to upscale.
    pub(super) fn run_post_fx(&mut self, view: &mut RenderView, ctx: &CameraCtx) {
        use crate::render::postfx::PostFxContext;

        let (scene, stack) = (ctx.scene, ctx.stack);
        let effect = EffectCx {
            post: stack.post,
            graphics: self.graphics(stack.editor_mode),
            full: stack.full_post_fx,
        };
        let pass = PassCx {
            scene,
            stack,
            effect,
        };
        for run in effects::scene_passes() {
            run(self, view, &pass);
        }

        // The grade + tonemap and the Color Lookup (#1440, #1441): rebaked only when
        // this view's inputs changed.
        let (gpu, bake) = ((&self.device, &self.queue), &mut self.grade_bake);
        let baked = view.post_fx.grade.prepare(gpu, bake, effect.volume());
        self.frame_counters.grade_lut_bakes += u32::from(baked);

        // The base camera's eye, as its view-projection is: an effect rebuilding
        // world positions reads both, and the last camera of a stack is an
        // overlay's (#1526).
        let eye = stack.stack[0].position.to_array();
        let params = frame_params(scene, ctx.base_view_proj, eye);
        let sky = self.sky_panorama();
        let chain = PostFxContext {
            depth_view: &view.depth_view,
            skybox: (&sky.view, &sky.sampler),
            output: stack.output,
            timer: &self.gpu_timer,
            passes: &self.post_passes,
        };
        view.post_fx
            .run((&self.device, &self.queue), chain, &effect, params);
    }
}

/// The base camera's TAA jitter-scale and mip-bias knobs (#1847); a view with no
/// camera keeps URP's defaults, so it jitters and samples as before.
fn taa_knobs(camera: Option<&crate::components::CameraComponent>) -> (f32, f32) {
    camera.map_or((1.0, 0.0), |c| (c.taa_jitter_scale, c.taa_mip_bias))
}

/// Each camera's view-projection at `aspect`, and beside it each one's entity, which
/// keeps its motion history (#1435).
fn matrices(pass: &StackPass, aspect: f32) -> (Vec<glam::Mat4>, Vec<(Option<u32>, glam::Mat4)>) {
    let view_projs: Vec<_> = (pass.stack.iter())
        .map(|c| c.build_view_projection(aspect))
        .collect();
    let cameras = pass.entities.iter().copied();
    let cameras = cameras.zip(view_projs.iter().copied()).collect();
    (view_projs, cameras)
}

/// The velocity target a view writes: motion vectors when an effect reads motion,
/// SSR's depths alone when only SSR runs, else none (#1435, #1526).
fn velocity_target(moving: bool, ssr: bool) -> Velocity {
    match (moving, ssr) {
        (true, _) => Velocity::Motion,
        (false, true) => Velocity::Depths,
        (false, false) => Velocity::Off,
    }
}
