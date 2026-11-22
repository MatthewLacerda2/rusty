use super::super::{Composite, EffectCx, History, Owner, Phase, PostEffect};
use crate::postfx::params::Gate;
use rusty_core::settings::graphics_options::GraphicsOption;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "chromatic_aberration",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::ChromaticAberration),
    history: History::None("a function of this frame's colour"),
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
    passes: &[],
};

fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let ca = &vc.chromatic_aberration;
    if cx.runs(EFFECT.gate, ca.active) {
        slots[0][0] = ca.intensity.clamp(0.0, 1.0);
    }
}
