//! White balance's ops: temperature and tint, each `-100..=100`, set together.

use super::super::{Effect, Knob};
use crate::components::{PostProcessingComponent, WhiteBalance};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::clamped;

/// Set the white balance: `temperature` and `tint`, each clamped to `-100..=100`.
pub fn set(vc: &mut PostProcessingComponent, temperature: f32, tint: f32) {
    let r = WhiteBalance::RANGE;
    vc.white_balance = WhiteBalance {
        temperature: clamped(temperature, -r, r),
        tint: clamped(tint, -r, r),
    };
}

pub const EFFECT: Effect = Effect {
    name: "white_balance",
    field: effect!(white_balance, |c, v: WhiteBalance| set(
        c,
        v.temperature,
        v.tint
    )),
    knobs: &[
        Knob {
            field: "temperature",
            api: None,
            get: |vc| vc.white_balance.temperature,
            set: |vc, t| set(vc, t, vc.white_balance.tint),
        },
        Knob {
            field: "tint",
            api: None,
            get: |vc| vc.white_balance.tint,
            set: |vc, t| set(vc, vc.white_balance.temperature, t),
        },
    ],
};
