//! Chromatic aberration (#1439): a composite function on the HDR image, before
//! bloom and the grade.

use super::super::{Composite, EffectCx, Owner, Phase, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::render::postfx::params::Gate;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "chromatic_aberration",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::ChromaticAberration),
    module: Some("postfx/effects/chromatic_aberration.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 10,
        function: "hdr",
        slots: 1,
        textures: &[],
        pack,
    }],
    grade: &[],
    pass: None,
};

/// x: the intensity, clamped (a loaded file bypasses the op's clamp); `0` when it
/// doesn't run, which the shader skips.
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let ca = &vc.chromatic_aberration;
    if cx.runs(EFFECT.gate, ca.active) {
        slots[0][0] = ca.intensity.clamp(0.0, 1.0);
    }
}
