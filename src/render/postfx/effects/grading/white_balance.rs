//! White balance (#1440): the bake's first function, Unity's LMS white-point scale.

use glam::{Mat3, Vec3};

use super::super::{Grade, GradeCx, Owner, PostEffect};
use super::bounded;
use crate::components::WhiteBalance;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "white_balance",
    owner: Owner::Volume,
    option: None,
    module: Some("postfx/effects/white_balance.wgsl"),
    composite: &[],
    grade: &[Grade {
        order: 10,
        function: "grade",
        slots: 1,
        pack,
    }],
    pass: None,
};

/// xyz: the LMS coefficients, w: `1` when white balance runs (neutral skips it).
fn pack(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0] = [1.0, 1.0, 1.0, 0.0];
    let Some(vc) = cx.volume else { return };
    let r = WhiteBalance::RANGE;
    let balance = WhiteBalance {
        temperature: bounded(vc.white_balance.temperature, r),
        tint: bounded(vc.white_balance.tint, r),
    };
    let on = if balance.is_neutral() { 0.0 } else { 1.0 };
    slots[0] = coefficients(balance).extend(on).to_array();
}

/// Unity's `ColorUtils.ColorBalanceToLMSCoeffs`: the per-channel LMS scale that moves
/// D65 to the white point `temperature` and `tint` name.
pub(crate) fn coefficients(wb: WhiteBalance) -> Vec3 {
    if wb.is_neutral() {
        return Vec3::ONE;
    }
    let t1 = wb.temperature / 65.0;
    let t2 = wb.tint / 65.0;
    // The reference white's CIE xy: D65's x, moved along the daylight locus.
    let x = 0.31271 - t1 * if t1 < 0.0 { 0.1 } else { 0.05 };
    let y = 2.87 * x - 3.0 * x * x - 0.275_095_07 + t2 * 0.05;
    let d65 = Vec3::new(0.949_237, 1.035_42, 1.087_28);
    d65 / xy_to_lms(x, y)
}

/// CIE xy (at `Y = 1`) to LMS, through CIE XYZ.
fn xy_to_lms(x: f32, y: f32) -> Vec3 {
    let xyz = Vec3::new(x / y, 1.0, (1.0 - x - y) / y);
    let to_lms = Mat3::from_cols(
        Vec3::new(0.7328, -0.7036, 0.0030),
        Vec3::new(0.4296, 1.6975, 0.0136),
        Vec3::new(-0.1624, 0.0061, 0.9834),
    );
    to_lms * xyz
}
