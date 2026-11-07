//! TAA (#1702) as a post effect: a **camera** effect (Unity's camera anti-aliasing
//! mode), its pass a scene pass after depth of field (10) and before motion blur
//! (20), URP's order. Like FXAA it needs no volume.

use super::super::{Owner, Pass, PassCx, PostEffect};
use crate::components::ExposureMode;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::upscaler::Upscaler;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::Gate;
use crate::render::postfx::params::{effect_runs, PostSource};
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "taa",
    owner: Owner::Camera,
    gate: Gate::option(GraphicsOption::Taa),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[
        Pass::Scene {
            order: 15,
            run: resolve,
        },
        super::sharpen::PASS,
    ],
};

/// The resolve, TAA's or FSR2's in its place (#1848); [`super::begin`] already
/// chose whether and how this view runs it, and held the history only then.
fn resolve(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let gpu = (&r.device, &r.queue);
    let lenses = super::params::lenses(cx.stack.stack, view.aspect());
    let history = view.post_fx.targets.get::<super::TaaHistory>();
    if history.is_some_and(|h| h.resolver == Upscaler::Fsr2) {
        let inputs = super::fsr2::Fsr2Inputs {
            exposure: exposure(cx),
            lens: [lenses[0][0], lenses[0][1]],
        };
        let passes = &r.post_passes.taa.fsr2;
        return view.post_fx.run_fsr2(gpu, passes, &r.gpu_timer, inputs);
    }
    // The base camera's TAA knobs (#1847); a view with no camera keeps URP's
    // defaults, so its resolve is unchanged.
    let tune = cx
        .effect
        .post
        .camera
        .map(super::params::TaaTune::from_camera)
        .unwrap_or_default();
    view.post_fx
        .run_taa(gpu, &r.post_passes.taa, &r.gpu_timer, tune, lenses);
}

/// The exposure FSR2 reads (#1848): the volume's fixed EV, and whether its mode is
/// Automatic (the meter's adapted EV then runs); EV 0 without a volume.
fn exposure(cx: &PassCx) -> (f32, bool) {
    cx.effect.volume().map_or((0.0, false), |vc| {
        let auto = vc.exposure.mode == ExposureMode::Automatic;
        (vc.exposure.fixed_ev, auto)
    })
}

/// Whether `post`'s camera runs TAA, through [`effect_runs`]: its component has it
/// on and the player's option allows it.
pub(crate) fn taa_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    post.camera
        .is_some_and(|cam| effect_runs(graphics, EFFECT.gate, cam.taa_active))
}

/// Whether a view runs TAA this frame: `full` post-FX (not a display-only render
/// texture) and not the editor's Scene view, whose camera is the editor's and which
/// keeps FXAA (URP runs no TAA in its Scene view either).
pub(crate) fn wanted(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    full && !scene_view && taa_enabled(post, graphics)
}

/// Whether `view` runs TAA this frame: [`wanted`], and the view shades with the
/// engine's own forward pipeline (the Inspector's shader preview does not). The one
/// predicate `draw_stack` and `Renderer::render` (which picks where a scaled view
/// composites before the stack draws) both ask.
pub(crate) fn runs(
    view: &RenderView,
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    wanted(post, graphics, full, scene_view) && !view.has_forward_override()
}
