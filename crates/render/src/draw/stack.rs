use super::camera::{flushes_after, CameraCtx, StackShared, CAMERA_PASSES};
use crate::gpu::commands::Commands;
use crate::gpu::uniforms::FogUniform;
use crate::motion::{self, HistoryKey, Velocity};
use crate::passes::ssao::SsaoPlan;
use crate::postfx::effects::ssil;
use crate::postfx::effects::ssr::{ssr_enabled, surface, transparent};
use crate::postfx::effects::{self, taa, volumetric_fog, EffectCx, PassCx};
use crate::postfx::params::{frame_params, PostSource};
use crate::{RenderView, Renderer};
use rusty_core::settings::GraphicsSettings;
use rusty_scene::{Camera, Scene};

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

pub(crate) struct StackPass<'a> {
    pub stack: &'a [Camera],
    pub entities: &'a [Option<u32>],
    pub base_id: Option<u32>,
    pub post: PostSource<'a>,
    pub output: &'a wgpu::TextureView,
    pub editor_mode: bool,
    pub full_post_fx: bool,
    pub motion_frames: u32,
}

impl Renderer {
    pub(crate) fn draw_stack(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        (pass, commands): (StackPass, Commands),
    ) -> Commands {
        let aspect = view.aspect();
        let (view_projs, cameras) = matrices(&pass, aspect);
        let graphics = self.graphics(pass.editor_mode);
        let own_pipeline = view.has_forward_override();
        let ssr = ssr_enabled(pass.post, graphics, pass.full_post_fx) && !own_pipeline;
        let moving =
            motion::target_wanted(pass.post, graphics, pass.full_post_fx, pass.editor_mode)
                && !own_pipeline;
        let velocity = velocity_target(moving, ssr);
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
        let keeps = !own_pipeline && !self.capture.static_only;
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
        let commands = self.draw_cameras(view, scene, (&pass, &shared), commands);
        view.post_fx.motion.history.end();
        commands
    }

    fn draw_cameras(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        (pass, shared): (&StackPass, &StackShared),
        mut commands: Commands,
    ) -> Commands {
        for idx in 0..pass.stack.len() {
            if idx > 0 {
                commands.flush();
                self.lap("camera.submit");
            }
            let mut ctx = CameraCtx::new(scene, pass, shared, (idx, commands));
            self.lap("stack");
            let flushes = self.switches.flushes();
            for (name, stage) in CAMERA_PASSES {
                stage(self, view, &mut ctx);
                #[cfg(test)]
                if super::camera::split_stages() {
                    ctx.commands.flush();
                }
                self.lap(name);
                if flushes_after(flushes, name) {
                    ctx.commands.flush();
                    self.lap("camera.flush");
                }
            }
            commands = ctx.commands;
        }
        commands
    }

    pub(super) fn run_post_fx(&mut self, view: &mut RenderView, ctx: &mut CameraCtx) {
        use crate::postfx::PostFxContext;

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
        let commands = &mut ctx.commands;
        for run in effects::scene_passes() {
            run(self, view, &pass, commands);
        }

        let gpu = (&self.device, (&self.queue, &self.staging));
        let bake = &mut self.grade_bake;
        let baked = view.post_fx.grade.prepare(gpu, bake, effect.volume());
        self.frame_counters.grade_lut_bakes += u32::from(baked);

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
        view.post_fx.run(
            (&self.device, &self.queue),
            commands,
            chain,
            (&effect, params),
        );
    }
}

fn taa_knobs(camera: Option<&rusty_components::CameraComponent>) -> (f32, f32) {
    camera.map_or((1.0, 0.0), |c| (c.taa_jitter_scale, c.taa_mip_bias))
}

fn matrices(pass: &StackPass, aspect: f32) -> (Vec<glam::Mat4>, Vec<(Option<u32>, glam::Mat4)>) {
    let view_projs: Vec<_> = (pass.stack.iter())
        .map(|c| c.build_view_projection(aspect))
        .collect();
    let cameras = pass.entities.iter().copied();
    let cameras = cameras.zip(view_projs.iter().copied()).collect();
    (view_projs, cameras)
}

fn velocity_target(moving: bool, ssr: bool) -> Velocity {
    match (moving, ssr) {
        (true, _) => Velocity::Motion,
        (false, true) => Velocity::Depths,
        (false, false) => Velocity::Off,
    }
}
