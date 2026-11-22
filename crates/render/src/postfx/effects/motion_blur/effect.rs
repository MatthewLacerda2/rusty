use super::super::{History, Owner, Pass, PassCx, PostEffect};
use super::BlurParams;
use crate::gpu::commands::Commands;
use crate::motion;
use crate::postfx::params::Gate;
use crate::postfx::params::{effect_runs, PostSource};
use crate::{RenderView, Renderer};
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::GraphicsSettings;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "motion_blur",
    owner: Owner::Camera,
    gate: Gate::option(GraphicsOption::MotionBlur),
    history: History::None(
        "its velocities are the motion vectors, counted where they are recorded",
    ),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[Pass::Scene {
        order: 20,
        run: blur,
    }],
};

/// Gated by `motion::wanted`: the Scene view never blurs, nor a view that wrote velocity only
/// for depth of field.
fn blur(r: &Renderer, view: &mut RenderView, cx: &PassCx, commands: &mut Commands) {
    let (post, graphics, stack) = (cx.effect.post, cx.effect.graphics, cx.stack);
    let blurs = motion::wanted(post, graphics, cx.effect.full, stack.editor_mode);
    let blur = blur_params(post, graphics, view.post_fx.chain_size.1)
        .filter(|_| blurs)
        .map(|b| b.per_frames(stack.motion_frames));
    let gpu = (&r.device, &r.queue);
    let passes = &r.post_passes.motion_blur;
    view.post_fx
        .run_motion_blur(gpu, commands.encoder(), passes, blur, &r.gpu_timer);
}

pub(crate) fn blur_params(
    post: PostSource,
    graphics: GraphicsSettings,
    height: u32,
) -> Option<BlurParams> {
    let cam = post
        .camera
        .filter(|_| motion_blur_enabled(post, graphics))?;
    let samples = cam.motion_blur_quality.pick(
        &graphics.quality.motion_blur_samples,
        cam.motion_blur_samples,
    );
    Some(BlurParams::new(cam, samples, height))
}

pub(crate) fn motion_blur_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    post.camera
        .is_some_and(|cam| effect_runs(graphics, EFFECT.gate, cam.motion_blur_active))
}
