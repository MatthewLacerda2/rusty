//! The vignette's ops: the switch, the strength, the falloff, the colour and the
//! centre. Each axis of the centre is a knob of its own, the other axis kept.

use super::super::{Effect, Knob};
use crate::components::{PostProcessingComponent, Vignette};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::{clamped, finite_array_or, unit};

/// Set whether the vignette draws.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.vignette.active = active;
}

/// Set how strongly the vignette darkens the edges, clamped to `0..=1`.
pub fn set_intensity(vc: &mut PostProcessingComponent, value: f32) {
    vc.vignette.intensity = unit(value);
}

/// Set the vignette's falloff softness, clamped to `0.01..=1`.
pub fn set_smoothness(vc: &mut PostProcessingComponent, value: f32) {
    vc.vignette.smoothness = clamped(value, Vignette::MIN_SMOOTHNESS, 1.0);
}

/// Set the colour the vignette fades to, each channel clamped to `0..=1` (a colour
/// with a NaN or infinite channel keeps the current one).
pub fn set_color(vc: &mut PostProcessingComponent, color: [f32; 3]) {
    vc.vignette.color = finite_array_or(color, vc.vignette.color).map(unit);
}

/// Set the vignette's clear centre in screen uv, each axis clamped to `0..=1` (a
/// point with a NaN or infinite axis keeps the current one).
pub fn set_center(vc: &mut PostProcessingComponent, center: [f32; 2]) {
    vc.vignette.center = finite_array_or(center, vc.vignette.center).map(unit);
}

pub const EFFECT: Effect = Effect {
    name: "vignette",
    field: effect!(vignette, |c, v: Vignette| {
        set_active(c, v.active);
        set_intensity(c, v.intensity);
        set_smoothness(c, v.smoothness);
        set_color(c, v.color);
        set_center(c, v.center);
    }),
    knobs: &[
        Knob {
            field: "intensity",
            api: Some("VignetteIntensity"),
            get: |vc| vc.vignette.intensity,
            set: set_intensity,
        },
        Knob {
            field: "smoothness",
            api: Some("VignetteSmoothness"),
            get: |vc| vc.vignette.smoothness,
            set: set_smoothness,
        },
        Knob {
            field: "center.x",
            api: None,
            get: |vc| vc.vignette.center[0],
            set: |vc, x| set_center(vc, [x, vc.vignette.center[1]]),
        },
        Knob {
            field: "center.y",
            api: None,
            get: |vc| vc.vignette.center[1],
            set: |vc, y| set_center(vc, [vc.vignette.center[0], y]),
        },
    ],
};
