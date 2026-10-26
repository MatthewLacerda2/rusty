//! Tonemapping (#1440): the operator, then, last in the grade before the Color Lookup,
//! the display-gamma tweak.

use super::super::{Grade, GradeCx, Owner, PostEffect};
use crate::components::Tonemap;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "tonemapping",
    owner: Owner::Volume,
    option: None,
    module: Some("postfx/effects/tonemapping.wgsl"),
    composite: &[],
    grade: &[
        Grade {
            order: 40,
            function: "tonemap",
            slots: 1,
            pack: pack_tonemap,
        },
        Grade {
            order: 60,
            function: "display_gamma",
            slots: 1,
            pack: pack_gamma,
        },
    ],
    pass: None,
};

/// x: the operator's index; none (clamp only) without a volume.
fn pack_tonemap(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    let mode = cx.volume.map_or(Tonemap::None, |vc| vc.tonemapping.mode);
    slots[0][0] = mode.to_index() as f32;
}

/// x: the display gamma, floored as the op floors it; neutral `1`.
fn pack_gamma(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0][0] = cx
        .volume
        .map_or(1.0, |vc| vc.tonemapping.display_gamma.max(0.01));
}
