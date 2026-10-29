//! Screen-space reflections' ops: the switch and the quality name. No knobs: neither
//! is a number.

use super::Effect;
use crate::components::{PostProcessingComponent, Ssr};
use crate::scene::authoring::fields::table::effect;

/// Set whether SSR is active.
pub fn set_active(vc: &mut PostProcessingComponent, active: bool) {
    vc.ssr.active = active;
}

/// Set the SSR quality preset name (`"Low"`/`"Medium"`/`"High"`/`"Ultra"`).
pub fn set_quality(vc: &mut PostProcessingComponent, quality: String) {
    vc.ssr.quality = quality;
}

pub const EFFECT: Effect = Effect {
    name: "ssr",
    field: effect!(ssr, |c, v: Ssr| {
        set_active(c, v.active);
        set_quality(c, v.quality);
    }),
    knobs: &[],
};
