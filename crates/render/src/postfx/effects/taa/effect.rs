//! TAA (#1702) as a post effect: a **camera** effect (Unity's camera anti-aliasing
//! mode), its pass a scene pass after depth of field (10) and before motion blur
//! (20), URP's order. Like FXAA it needs no volume.

use super::super::{History, Owner, Pass, PassCx, PostEffect};
use crate::postfx::params::Gate;
use crate::postfx::params::{effect_runs, PostSource};
use crate::{RenderView, Renderer};
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::GraphicsSettings;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "taa",
    owner: Owner::Camera,
    gate: Gate::option(GraphicsOption::Taa),
    history: History::Reads(|cx| cx.taa),
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

/// The resolve; [`super::begin`] already chose whether this view runs it, and held
/// the history only then.
fn resolve(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let gpu = (&r.device, &r.queue);
    let lenses = super::params::lenses(cx.stack.stack, view.aspect());
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
