//! Screen-space reflections: a composite function on the HDR image (High tier only;
//! Low and Medium keep the forward shader's cubemap reflection, which the volume's
//! SSR settings also steer, `render::draw::lighting`).

use super::{Composite, EffectCx, Owner, Phase, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::QualityPreset;
use crate::render::postfx::params::Gate;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssr",
    owner: Owner::Volume,
    gate: Gate {
        option: Some(GraphicsOption::Ssr),
        tiers: QualityPreset::screen_space_ssr,
    },
    module: Some("postfx/effects/ssr.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 20,
        function: "hdr",
        slots: 1,
        textures: &[],
        pack,
    }],
    grade: &[],
    pass: None,
};

/// x: the mode, `2` (screen-space) when it runs, else `0`.
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    if cx.runs(EFFECT.gate, vc.ssr.active) {
        slots[0][0] = 2.0;
    }
}
