//! Screen-space reflections' ops: the switch, the quality name and temporal
//! upsampling. No knobs: none of them is a number.

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

/// Set whether SSR temporal upsampling is on.
pub fn set_temporal_upsampling(vc: &mut PostProcessingComponent, on: bool) {
    vc.ssr.temporal_upsampling = on;
}

pub const EFFECT: Effect = Effect {
    name: "ssr",
    field: effect!(ssr, |c, v: Ssr| {
        set_active(c, v.active);
        set_quality(c, v.quality);
        set_temporal_upsampling(c, v.temporal_upsampling);
    }),
    knobs: &[],
};
