//! Screen-space reflections (where it is off, the forward pass reflects the sky or a
//! reflection probe, #718): a pass of its own ([`pass`]) over the base
//! camera's opaque image, right after its opaque pass, so the translucents, the
//! overlay cameras, depth of field and motion blur all see the reflected scene (HDRP's
//! order). It is in no post-chain stage: `render::draw::camera`'s `ssr` stage runs it.
//!
//! It reads three per-view targets (#1526): the velocity target (#1435), for each
//! pixel's depth, and the two SSR surface targets the opaque pass writes while SSR
//! runs ([`surface`]): the shading normal, the weight a hit gets and the environment
//! reflection it replaces. So only a smooth surface reflects, and a hit swaps the sky
//! or probe reflection for what is on screen (never adding to it). The volume's
//! `quality` sets the march ([`march`], #1521, #1522). A transparent surface that
//! opts in traces its own reflection in the transparent pass ([`transparent`], #1570).

pub(crate) mod pass;
pub(crate) mod surface;
pub(crate) mod transparent;

use super::{Owner, PostEffect};
use crate::components::Ssr;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::SsrQuality;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::{effect_runs, Gate, PostSource};
use crate::render::{RenderView, Renderer};

pub(crate) use pass::{SsrParams, SsrPasses};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssr",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Ssr),
    module: None,
    composite: &[],
    grade: &[],
    pass: None,
};

/// The march `ssr` runs under `graphics`: its quality level's row of the running
/// preset, or its own `steps` and `step_length` when `Custom`.
pub(crate) fn march(ssr: &Ssr, graphics: GraphicsSettings) -> SsrQuality {
    let custom = SsrQuality {
        steps: ssr.steps,
        step_length: ssr.step_length,
    };
    ssr.quality.pick(&graphics.quality.ssr, custom)
}

/// Whether a view whose base camera is `post`'s runs SSR this frame: its volume has
/// it on and the player's option allows it. `full` is the stack's
/// `full_post_fx`. The opaque pass writes the surface targets exactly then.
pub(crate) fn ssr_enabled(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    let on = post.volume.is_some_and(|vc| vc.ssr.active);
    full && effect_runs(graphics, EFFECT.gate, on)
}

/// Reflect the base camera's opaque image when the view runs SSR, else drop its
/// state. `post` is the stack's base camera's settings, `full` its `full_post_fx`,
/// and `view_proj` and `eye` the base camera's.
pub(crate) fn run(
    r: &Renderer,
    view: &mut RenderView,
    (post, graphics, full): (PostSource, GraphicsSettings, bool),
    (view_proj, eye): (glam::Mat4, glam::Vec3),
) {
    let params = post
        .volume
        .filter(|_| ssr_enabled(post, graphics, full))
        .map(|vc| SsrParams::new(view_proj, eye, march(&vc.ssr, graphics)));
    let gpu = (&r.device, &r.queue);
    view.post_fx
        .run_ssr(gpu, &r.post_passes.ssr, params, &r.gpu_timer);
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transparent_tests;

#[cfg(test)]
mod gpu;
