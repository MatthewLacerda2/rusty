//! Screen-space reflections (where it is off, the forward pass reflects the sky or a
//! reflection probe, #718): a pass of its own ([`pass`]) over the base
//! camera's opaque image, right after its opaque pass, so the translucents, the
//! overlay cameras, depth of field and motion blur all see the reflected scene (HDRP's
//! order). It is in no post-chain stage: `render::draw::camera`'s `ssr` stage runs it.
//!
//! It reads three per-view targets (#1526): the velocity target (#1435), for each
//! pixel's depth, and the two SSR surface targets the opaque pass writes while SSR
//! runs ([`surface`]): the shading normal, the weight a hit gets and the environment
//! reflection it replaces, with the roughness packed in. So a surface smoother than
//! the volume's `min_smoothness` reflects, and a hit swaps the sky or probe
//! reflection for what is on screen (never adding to it), averaged over the disk its
//! GGX lobe spreads a reflection across ([`pass`]'s resolve, #2028, reading mips
//! of the hits, [`pyramid`], #1736): glossy SSR, a rough floor showing a soft
//! reflection whose silhouette fades across its blur. The volume's `quality` sets the march
//! ([`march`], #1521, #1522). A transparent surface that opts in traces its own
//! reflection in the transparent pass ([`transparent`], #1570).

pub(crate) mod pass;
pub(crate) mod pyramid;
mod record;
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
    passes: &[],
};

/// The march `ssr` runs under `graphics`: its quality level's row of the running
/// preset, or its own `steps` and `step_length` when `Custom`, within the march's
/// range (a preset in `project.rusty` is never clamped on the way in), so every
/// caller marches clamped values.
pub(crate) fn march(ssr: &Ssr, graphics: GraphicsSettings) -> SsrQuality {
    let custom = SsrQuality {
        steps: ssr.steps,
        step_length: ssr.step_length,
    };
    let m = ssr.quality.pick(&graphics.quality.ssr, custom);
    SsrQuality {
        steps: m.steps.clamp(1, Ssr::MAX_STEPS),
        step_length: m
            .step_length
            .clamp(Ssr::MIN_STEP_LENGTH, Ssr::MAX_STEP_LENGTH),
    }
}

/// HDRP's default *Smoothness Fade Start*: SSR is whole from this smoothness
/// whatever the *Minimum Smoothness* below it, so `min_smoothness` 0.8 fades in over
/// 0.1 exactly as SSR did before it was a knob (#1736).
const FADE_START: f32 = 0.9;

/// The narrowest fade, for a *Minimum Smoothness* at or above [`FADE_START`]:
/// nearly a hard cut, never a division by zero.
const HARD_FADE: f32 = 0.01;

/// The smoothest a surface gets: the forward pass clamps roughness to at least
/// 0.04 (`shader.wgsl`, `common.wgsl`'s decals). A *Minimum Smoothness* this high
/// reflects nothing, so the view runs no SSR and builds no pyramid.
pub(crate) const SMOOTHEST: f32 = 0.96;

/// `[min, width]`: the *Minimum Smoothness* `min_smoothness`, within `0..=1`, and
/// the smoothness over which SSR fades in above it, what `ssr_surface.wgsl`'s
/// `smoothness_fade` divides by: `Ssr::SMOOTHNESS_FADE`, narrowed so SSR is whole
/// by [`FADE_START`], and never under [`HARD_FADE`].
pub(crate) fn fade(min_smoothness: f32) -> [f32; 2] {
    let min = min_smoothness.clamp(0.0, 1.0);
    let width = (FADE_START - min).clamp(HARD_FADE, Ssr::SMOOTHNESS_FADE);
    [min, width]
}

/// Whether a view whose base camera is `post`'s runs SSR this frame: its volume has
/// it on, below [`SMOOTHEST`], and the player's option allows it. `full` is the
/// stack's `full_post_fx`. The opaque pass writes the surface targets exactly then.
pub(crate) fn ssr_enabled(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    let on = post
        .volume
        .is_some_and(|vc| vc.ssr.active && vc.ssr.min_smoothness < SMOOTHEST);
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
        .map(|vc| {
            let m = march(&vc.ssr, graphics);
            SsrParams::new(view_proj, eye, m, vc.ssr.min_smoothness)
        });
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
