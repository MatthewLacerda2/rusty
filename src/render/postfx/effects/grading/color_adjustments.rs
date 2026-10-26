//! Color adjustments (#1440): the hue shift before the tonemap, contrast and
//! saturation after it.

use super::super::{Grade, GradeCx, Owner, PostEffect};
use super::bounded;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "color_adjustments",
    owner: Owner::Volume,
    option: None,
    module: Some("postfx/effects/color_adjustments.wgsl"),
    composite: &[],
    grade: &[
        Grade {
            order: 30,
            function: "hue",
            slots: 1,
            pack: pack_hue,
        },
        Grade {
            order: 50,
            function: "contrast_saturation",
            slots: 1,
            pack: pack_contrast_saturation,
        },
    ],
    pass: None,
};

/// x: the hue shift in turns (`0` skips it).
fn pack_hue(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    if let Some(vc) = cx.volume {
        slots[0][0] = bounded(vc.color_adjustments.hue_shift, 180.0) / 360.0;
    }
}

/// x: contrast, y: saturation; neutral `1, 1`.
fn pack_contrast_saturation(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    let a = cx.volume.map(|vc| vc.color_adjustments);
    let (contrast, saturation) = a.map_or((1.0, 1.0), |a| (a.contrast, a.saturation));
    slots[0] = [contrast, saturation, 0.0, 0.0];
}
