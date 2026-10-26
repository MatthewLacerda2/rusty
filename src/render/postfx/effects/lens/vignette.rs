//! The vignette (#1439): a composite function on the graded image.

use super::super::{Composite, EffectCx, Owner, Phase, PostEffect};
use crate::components::Vignette;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::render::postfx::params::Gate;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "vignette",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Vignette),
    module: Some("postfx/effects/vignette.wgsl"),
    composite: &[Composite {
        phase: Phase::Display,
        order: 10,
        function: "display",
        slots: 2,
        pack,
    }],
    grade: &[],
    pass: None,
};

/// Slot 0: intensity (`0` when it doesn't run), smoothness, centre; slot 1: the
/// colour. Every knob is clamped to its range, since a loaded file bypasses the ops'
/// clamps: a zero smoothness would make the shader's `pow(0, 0)` a NaN.
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let v = &vc.vignette;
    let intensity = if cx.runs(EFFECT.gate, v.active) {
        v.intensity.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let smoothness = v.smoothness.clamp(Vignette::MIN_SMOOTHNESS, 1.0);
    slots[0] = [intensity, smoothness, v.center[0], v.center[1]];
    let [r, g, b] = v.color.map(|c| c.clamp(0.0, 1.0));
    slots[1] = [r, g, b, 0.0];
}
