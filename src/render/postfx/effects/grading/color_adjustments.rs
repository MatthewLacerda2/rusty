//! Color adjustments (#1440), all before the tonemap where Unity URP's grading LUT
//! runs them (#1479): contrast right after white balance, then the hue shift and
//! saturation after Lift / Gamma / Gain.

use super::super::{Grade, GradeCx, Owner, PostEffect};
use super::bounded;
use crate::components::ColorAdjustments;
use crate::render::postfx::params::Gate;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "color_adjustments",
    owner: Owner::Volume,
    gate: Gate::NONE,
    module: Some("postfx/effects/color_adjustments.wgsl"),
    composite: &[],
    grade: &[
        Grade {
            order: 15,
            function: "contrast",
            slots: 1,
            pack: pack_contrast,
        },
        Grade {
            order: 30,
            function: "hue",
            slots: 1,
            pack: pack_hue,
        },
        Grade {
            order: 35,
            function: "saturation",
            slots: 1,
            pack: pack_saturation,
        },
    ],
    pass: None,
};

/// The volume's adjustments, neutral without one.
fn adjustments(cx: &GradeCx) -> ColorAdjustments {
    cx.volume.map(|vc| vc.color_adjustments).unwrap_or_default()
}

/// x: contrast; neutral `1`.
fn pack_contrast(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0][0] = adjustments(cx).contrast;
}

/// x: the hue shift in turns (`0` skips it).
fn pack_hue(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0][0] = bounded(adjustments(cx).hue_shift, 180.0) / 360.0;
}

/// x: saturation; neutral `1`.
fn pack_saturation(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0][0] = adjustments(cx).saturation;
}
