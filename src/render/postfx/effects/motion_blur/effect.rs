//! Per-object motion blur as a post effect (#1436): a **camera** effect, its knobs on
//! the camera (Unity's camera property), its pass a scene pass over the velocity
//! target the stack wrote, after depth of field: bloom, the exposure meter and the
//! composite then read the blurred scene. Like FXAA it needs no volume.

use super::super::{Owner, Pass, PassCx, PostEffect};
use super::BlurParams;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::GraphicsSettings;
use crate::render::motion;
use crate::render::postfx::params::Gate;
use crate::render::postfx::params::{effect_runs, PostSource};
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "motion_blur",
    owner: Owner::Camera,
    gate: Gate {
        option: Some(GraphicsOption::MotionBlur),
        tiers: QualityPreset::motion_blur,
    },
    module: None,
    composite: &[],
    grade: &[],
    pass: Some(Pass::Scene {
        order: 20,
        run: blur,
    }),
};

/// Gated by [`motion::wanted`], so the editor's Scene view never blurs and a view
/// that wrote velocity only for depth of field runs no motion blur.
fn blur(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let (post, graphics, stack) = (cx.effect.post, cx.effect.graphics, cx.stack);
    let blurs = motion::wanted(post, graphics, cx.effect.full, stack.editor_mode);
    let blur = blur_params(post, graphics, view.size().height)
        .filter(|_| blurs)
        .map(|b| b.per_frames(stack.motion_frames));
    let gpu = (&r.device, &r.queue);
    view.post_fx
        .run_motion_blur(gpu, &r.post_passes.motion_blur, blur, &r.gpu_timer);
}

/// The motion blur `post`'s camera runs this frame, for a view `height` pixels tall;
/// `None` when [`motion_blur_enabled`] does not hold.
pub(crate) fn blur_params(
    post: PostSource,
    graphics: GraphicsSettings,
    height: u32,
) -> Option<BlurParams> {
    let cam = post
        .camera
        .filter(|_| motion_blur_enabled(post, graphics))?;
    Some(BlurParams::new(cam, height))
}

/// Whether `post`'s camera runs motion blur, through [`effect_runs`]: its component
/// has it on, the player's option allows it and the tier runs it. The velocity
/// target is written when this or depth of field holds (`motion::target_wanted`,
/// #1435, #1445), so a player who turned both off pays for no velocity pass.
pub(crate) fn motion_blur_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    post.camera
        .is_some_and(|cam| effect_runs(graphics, EFFECT.gate, cam.motion_blur_active))
}
