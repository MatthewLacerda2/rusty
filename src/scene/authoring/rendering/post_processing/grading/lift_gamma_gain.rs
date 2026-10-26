//! Lift / Gamma / Gain's ops: each wheel's colour and offset. Its knobs are the
//! three offsets, a wheel's brightness.

use super::super::{Effect, Knob};
use crate::components::{LiftGammaGain, PostProcessingComponent, Wheel};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::{clamped, finite_array_or, unit};

/// Set one wheel: its colour, each channel clamped to `0..=1` (a colour with a NaN
/// or infinite channel keeps the current one), and its offset, clamped to `-1..=1`.
pub fn set_wheel(vc: &mut PostProcessingComponent, which: Wheel, color: [f32; 3], offset: f32) {
    let wheel = vc.lift_gamma_gain.wheel_mut(which);
    wheel.color = finite_array_or(color, wheel.color).map(unit);
    wheel.offset = clamped(offset, -1.0, 1.0);
}

/// `which`'s offset, its colour kept.
fn set_offset(vc: &mut PostProcessingComponent, which: Wheel, offset: f32) {
    let color = vc.lift_gamma_gain.wheel(which).color;
    set_wheel(vc, which, color, offset);
}

pub const EFFECT: Effect = Effect {
    name: "lift_gamma_gain",
    field: effect!(lift_gamma_gain, |c, v: LiftGammaGain| {
        for w in Wheel::ALL {
            set_wheel(c, w, v.wheel(w).color, v.wheel(w).offset);
        }
    }),
    knobs: &[
        Knob {
            field: "lift.offset",
            api: None,
            get: |vc| vc.lift_gamma_gain.lift.offset,
            set: |vc, v| set_offset(vc, Wheel::Lift, v),
        },
        Knob {
            field: "gamma.offset",
            api: None,
            get: |vc| vc.lift_gamma_gain.gamma.offset,
            set: |vc, v| set_offset(vc, Wheel::Gamma, v),
        },
        Knob {
            field: "gain.offset",
            api: None,
            get: |vc| vc.lift_gamma_gain.gain.offset,
            set: |vc, v| set_offset(vc, Wheel::Gain, v),
        },
    ],
};
