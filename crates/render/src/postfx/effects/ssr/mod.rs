pub(crate) mod pass;
pub(crate) mod pyramid;
mod record;
pub(crate) mod surface;
mod targets;
pub(crate) mod transparent;

use super::{History, Owner, PostEffect};
use crate::gpu::commands::Commands;
use crate::postfx::params::{effect_runs, Gate, PostSource};
use crate::{RenderView, Renderer};
use rusty_components::Ssr;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::quality::{SsrQuality, MAX_RESOLUTION_DIVISOR};
use rusty_core::settings::GraphicsSettings;

pub(crate) use pass::{SsrParams, SsrPasses};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssr",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Ssr),
    history: History::None("traced from this frame's opaque image and depth"),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[],
};

/// Clamped here: a preset in `project.rusty` is never clamped on the way in.
pub(crate) fn march(ssr: &Ssr, graphics: GraphicsSettings) -> SsrQuality {
    let custom = SsrQuality {
        steps: ssr.steps,
        step_length: ssr.step_length,
        trace_resolution_divisor: ssr.trace_resolution_divisor,
    };
    let m = ssr.quality.pick(&graphics.quality.ssr, custom);
    SsrQuality {
        steps: m.steps.clamp(1, Ssr::MAX_STEPS),
        step_length: m
            .step_length
            .clamp(Ssr::MIN_STEP_LENGTH, Ssr::MAX_STEP_LENGTH),
        trace_resolution_divisor: m.trace_resolution_divisor.clamp(1, MAX_RESOLUTION_DIVISOR),
    }
}

// HDRP's default Smoothness Fade Start.
const FADE_START: f32 = 0.9;

/// The narrowest fade, for a minimum at or above `FADE_START`: never a division by zero.
const HARD_FADE: f32 = 0.01;

// The forward pass clamps roughness to 0.04; a minimum above this reflects nothing.
pub(crate) const SMOOTHEST: f32 = 0.96;

/// `[min, width]`: `ssr_surface.wgsl`'s `smoothness_fade` divides by the width.
pub(crate) fn fade(min_smoothness: f32) -> [f32; 2] {
    let min = min_smoothness.clamp(0.0, 1.0);
    let width = (FADE_START - min).clamp(HARD_FADE, Ssr::SMOOTHNESS_FADE);
    [min, width]
}

/// The opaque pass writes the surface targets when this holds and the view has no forward
/// override (`draw_stack`).
pub(crate) fn ssr_enabled(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    let on = post
        .volume
        .is_some_and(|vc| vc.ssr.active && vc.ssr.min_smoothness < SMOOTHEST);
    full && effect_runs(graphics, EFFECT.gate, on)
}

pub(crate) fn run(
    (r, commands): (&Renderer, &mut Commands),
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
        .run_ssr(gpu, commands, &r.post_passes.ssr, params, &r.gpu_timer);
}

#[cfg(test)]
mod poison;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transparent_tests;

#[cfg(test)]
mod gpu;
