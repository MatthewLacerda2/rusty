use crate::postfx::params::Gate;
use glam::Vec3;

use super::super::{Grade, GradeCx, History, Owner, PostEffect};
use super::bounded;
use rusty_components::{ColorWheel, LiftGammaGain, Wheel};
use rusty_core::color::luminance;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "lift_gamma_gain",
    owner: Owner::Volume,
    gate: Gate::NONE,
    history: History::None("a grade of this frame's colour"),
    module: Some("postfx/effects/lift_gamma_gain.wgsl"),
    composite: &[],
    grade: &[Grade {
        order: 20,
        function: "grade",
        slots: 3,
        pack,
    }],
    passes: &[],
};

fn pack(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume else {
        slots.copy_from_slice(&[[0.0; 4], [1.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 0.0]]);
        return;
    };
    let mut lgg = vc.lift_gamma_gain;
    for which in Wheel::ALL {
        let w = lgg.wheel_mut(which);
        w.color = w.color.map(|c| bounded(c - 0.5, 0.5) + 0.5);
        w.offset = bounded(w.offset, 1.0);
    }
    let (lift, inv_gamma, gain) = prepare(&lgg);
    slots[0] = lift.extend(0.0).to_array();
    slots[1] = inv_gamma.extend(0.0).to_array();
    slots[2] = gain.extend(0.0).to_array();
}

/// Unity's `ColorUtils.PrepareLiftGammaGain`; `postfx/grade.wgsl` mirrors its Rec. 709
/// luminance.
pub(crate) fn prepare(lgg: &LiftGammaGain) -> (Vec3, Vec3, Vec3) {
    let centred = |w: &ColorWheel| {
        let c = Vec3::from_array(w.color);
        c - luminance(c) + w.offset
    };
    let lift = centred(&lgg.lift);
    let gamma = centred(&lgg.gamma) + 1.0;
    let gain = centred(&lgg.gain) + 1.0;
    (lift, 1.0 / gamma.max(Vec3::splat(1e-3)), gain)
}
