//! TAA (#1702) as a post effect: a **camera** effect (Unity's camera anti-aliasing
//! mode), its pass a scene pass after depth of field (10) and before motion blur
//! (20), URP's order. Like FXAA it needs no volume.

use super::super::{Owner, Pass, PassCx, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
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
    pass: Some(Pass::Scene {
        order: 15,
        run: resolve,
    }),
};

/// The resolve; [`super::begin`] already chose whether this view runs it, and held
/// the history only then.
fn resolve(r: &Renderer, view: &mut RenderView, _: &PassCx) {
    let gpu = (&r.device, &r.queue);
    view.post_fx.run_taa(gpu, &r.post_passes.taa, &r.gpu_timer);
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
