use super::super::{History, Owner, Pass, PassCx, PostEffect};
use crate::gpu::commands::Commands;
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

fn resolve(r: &Renderer, view: &mut RenderView, cx: &PassCx, commands: &mut Commands) {
    let gpu = (&r.device, &r.queue);
    let lenses = super::params::lenses(cx.stack.stack, view.aspect());
    let tune = cx
        .effect
        .post
        .camera
        .map(super::params::TaaTune::from_camera)
        .unwrap_or_default();
    let encoder = commands.encoder();
    view.post_fx.run_taa(
        gpu,
        encoder,
        &r.post_passes.taa,
        &r.gpu_timer,
        (tune, lenses),
    );
}

pub(crate) fn taa_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    post.camera
        .is_some_and(|cam| effect_runs(graphics, EFFECT.gate, cam.taa_active))
}

pub(crate) fn wanted(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    full && !scene_view && taa_enabled(post, graphics)
}

/// The one predicate every caller asks (`draw_stack`, `Renderer::render`, the gizmos).
pub(crate) fn runs(
    view: &RenderView,
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    wanted(post, graphics, full, scene_view) && !view.has_forward_override()
}
