//! The exposure's ops (#1444): the mode, the fixed EV and the automatic mode's
//! limits, compensation, adaptation speeds and histogram window. The paired knobs
//! (the limits, the speeds, the window) are set together, each half clamped
//! against the other; a knob setting one half keeps the other.

use super::{Effect, Knob};
use crate::components::{AutoExposure, Exposure, ExposureMode, PostProcessingComponent};
use crate::scene::authoring::fields::table::effect;
use crate::scene::authoring::sanitize::{clamped, finite_or};

/// Set the fixed exposure (EV).
pub fn set_fixed_ev(vc: &mut PostProcessingComponent, value: f32) {
    vc.exposure.fixed_ev = finite_or(value, vc.exposure.fixed_ev);
}

/// Set where the exposure comes from: the fixed EV, or metered.
pub fn set_mode(vc: &mut PostProcessingComponent, mode: ExposureMode) {
    vc.exposure.mode = mode;
}

/// Set the automatic exposure's limits, each clamped to `±AutoExposure::EV_RANGE`;
/// `max_ev` is floored at `min_ev`.
pub fn set_limits(vc: &mut PostProcessingComponent, min_ev: f32, max_ev: f32) {
    let r = AutoExposure::EV_RANGE;
    let min_ev = clamped(min_ev, -r, r);
    vc.exposure.automatic.min_ev = min_ev;
    vc.exposure.automatic.max_ev = clamped(max_ev, min_ev, r);
}

/// Set the automatic exposure's compensation in EV, clamped to
/// `±AutoExposure::EV_RANGE`.
pub fn set_compensation(vc: &mut PostProcessingComponent, ev: f32) {
    let r = AutoExposure::EV_RANGE;
    vc.exposure.automatic.compensation = clamped(ev, -r, r);
}

/// Set how fast the automatic exposure rises and falls, in EV per second, each
/// clamped to `0..=AutoExposure::MAX_SPEED`.
pub fn set_adaptation_speed(vc: &mut PostProcessingComponent, up: f32, down: f32) {
    let max = AutoExposure::MAX_SPEED;
    vc.exposure.automatic.speed_up = clamped(up, 0.0, max);
    vc.exposure.automatic.speed_down = clamped(down, 0.0, max);
}

/// Set the histogram window the meter averages: `low` clamped to `0..=100`, `high`
/// to `low..=100`.
pub fn set_histogram_percent(vc: &mut PostProcessingComponent, low: f32, high: f32) {
    let low = clamped(low, 0.0, 100.0);
    vc.exposure.automatic.low_percent = low;
    vc.exposure.automatic.high_percent = clamped(high, low, 100.0);
}

fn auto(vc: &PostProcessingComponent) -> AutoExposure {
    vc.exposure.automatic
}

pub const EFFECT: Effect = Effect {
    name: "exposure",
    field: effect!(exposure, |c, v: Exposure| {
        set_fixed_ev(c, v.fixed_ev);
        set_mode(c, v.mode);
        let a = v.automatic;
        set_limits(c, a.min_ev, a.max_ev);
        set_compensation(c, a.compensation);
        set_adaptation_speed(c, a.speed_up, a.speed_down);
        set_histogram_percent(c, a.low_percent, a.high_percent);
    }),
    knobs: &[
        Knob {
            field: "fixed_ev",
            api: Some("Exposure"),
            get: |vc| vc.exposure.fixed_ev,
            set: set_fixed_ev,
        },
        Knob {
            field: "automatic.min_ev",
            api: None,
            get: |vc| auto(vc).min_ev,
            set: |vc, v| set_limits(vc, v, auto(vc).max_ev),
        },
        Knob {
            field: "automatic.max_ev",
            api: None,
            get: |vc| auto(vc).max_ev,
            set: |vc, v| set_limits(vc, auto(vc).min_ev, v),
        },
        Knob {
            field: "automatic.compensation",
            api: Some("ExposureCompensation"),
            get: |vc| auto(vc).compensation,
            set: set_compensation,
        },
        Knob {
            field: "automatic.speed_up",
            api: None,
            get: |vc| auto(vc).speed_up,
            set: |vc, v| set_adaptation_speed(vc, v, auto(vc).speed_down),
        },
        Knob {
            field: "automatic.speed_down",
            api: None,
            get: |vc| auto(vc).speed_down,
            set: |vc, v| set_adaptation_speed(vc, auto(vc).speed_up, v),
        },
        Knob {
            field: "automatic.low_percent",
            api: None,
            get: |vc| auto(vc).low_percent,
            set: |vc, v| set_histogram_percent(vc, v, auto(vc).high_percent),
        },
        Knob {
            field: "automatic.high_percent",
            api: None,
            get: |vc| auto(vc).high_percent,
            set: |vc, v| set_histogram_percent(vc, auto(vc).low_percent, v),
        },
    ],
};
