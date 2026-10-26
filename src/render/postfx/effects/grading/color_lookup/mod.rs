//! The Color Lookup (#1441): the bake's last function, the volume's user LUT
//! (`user`, uploaded as a 3D texture the bake binds) over the display colour, so the
//! composite still grades a pixel with one fetch.

mod user;

pub(crate) use user::neutral;
pub use user::{Lookup, UserLuts};

use super::super::{Grade, GradeCx, Owner, PostEffect};
use crate::render::postfx::params::Gate;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "color_lookup",
    owner: Owner::Volume,
    gate: Gate::NONE,
    module: Some("postfx/effects/color_lookup.wgsl"),
    composite: &[],
    grade: &[Grade {
        order: 70,
        function: "grade",
        slots: 2,
        pack,
    }],
    pass: None,
};

/// Slot 0: the LUT's input range start (xyz) and its contribution held to `0..=1`
/// (w; `0`, none, when no LUT applies or it is NaN); slot 1: the range end.
fn pack(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    let Some(([lo, hi], contribution)) = cx.lookup else {
        slots.copy_from_slice(&[[0.0; 4], [1.0, 1.0, 1.0, 0.0]]);
        return;
    };
    let k = if contribution.is_nan() {
        0.0
    } else {
        contribution.clamp(0.0, 1.0)
    };
    slots[0] = [lo[0], lo[1], lo[2], k];
    slots[1] = [hi[0], hi[1], hi[2], 0.0];
}
