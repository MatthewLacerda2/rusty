mod history;
pub(crate) mod pass;
pub(crate) mod pyramid;
pub(crate) mod rays;
mod record;
pub(crate) mod surface;
mod targets;
pub(crate) mod transparent;

use super::{History, Owner, PostEffect};
use crate::gpu::commands::Commands;
use crate::gpu::uniforms::FogUniform;
use crate::postfx::params::{effect_runs, Gate, PostSource};
use crate::{RenderView, Renderer};
use rusty_components::{Ssr, SsrAlgorithm};
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::quality::{SsrQuality, MAX_RESOLUTION_DIVISOR};
use rusty_core::settings::GraphicsSettings;

pub(crate) use pass::{Rays, SsrParams, SsrPasses};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssr",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Ssr),
    history: History::Reads(|cx| accumulates(&cx.effect)),
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

fn accumulates(cx: &super::EffectCx) -> bool {
    ssr_accumulates(cx.post, cx.graphics, cx.full)
}

pub(crate) fn ssr_accumulates(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    let algorithm = post.volume.map(|vc| vc.ssr.algorithm);
    algorithm == Some(SsrAlgorithm::PbrAccumulation) && ssr_enabled(post, graphics, full)
}

/// Returns the ray texels shaded (`groups`: the frame's tree and shading group).
pub(crate) fn run(
    (r, commands): (&Renderer, &mut Commands),
    view: &mut RenderView,
    (post, graphics, full): (PostSource, GraphicsSettings, bool),
    at: (glam::Mat4, glam::Vec3, FogUniform),
    groups: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
) -> u64 {
    let params = post
        .volume
        .filter(|_| ssr_enabled(post, graphics, full))
        .map(|vc| {
            let m = march(&vc.ssr, graphics);
            let accumulate = vc.ssr.algorithm == SsrAlgorithm::PbrAccumulation;
            SsrParams::new(at, m, vc.ssr.min_smoothness, accumulate)
        });
    let rays = ray_frame(post, graphics, full)
        .zip(groups)
        .map(|(params, (tree, shade))| Rays {
            tree,
            shade,
            params,
        });
    let gpu = (&r.device, &r.queue);
    view.post_fx.run_ssr(
        gpu,
        commands,
        &r.post_passes.ssr,
        (params, rays),
        &r.gpu_timer,
    )
}

pub(crate) fn ray_frame(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
) -> Option<[f32; 4]> {
    let vc = post.volume.filter(|_| rays::traces(post, graphics, full))?;
    Some(ray_params(&vc.ssr, march(&vc.ssr, graphics)))
}

/// `SsrParams::rays`: the smoothness a ray needs, its reach, its divisor, and 1 (on).
fn ray_params(ssr: &Ssr, m: SsrQuality) -> [f32; 4] {
    let length = ssr
        .ray_length
        .clamp(Ssr::MIN_RAY_LENGTH, Ssr::MAX_RAY_LENGTH);
    let length = if length.is_finite() {
        length
    } else {
        Ssr::MIN_RAY_LENGTH
    };
    let divisor = m.trace_resolution_divisor as f32;
    [ssr.ray_threshold(), length, divisor, 1.0]
}

#[cfg(test)]
mod poison;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transparent_tests;

#[cfg(test)]
mod gpu;
