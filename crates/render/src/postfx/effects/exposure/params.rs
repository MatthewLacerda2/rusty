use crate::motion::HistoryKey;
use rusty_components::AutoExposure;

/// Mirrors `Params` in `postfx/exposure.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExposureParams {
    /// min_ev, max_ev, compensation, unused
    pub limits: [f32; 4],
    /// speed_up, speed_down (EV per second), dt (game seconds), snap (1: no easing)
    pub adapt: [f32; 4],
    /// low, high (fractions of the samples), unused, unused
    pub window: [f32; 4],
}

impl ExposureParams {
    pub fn new(ae: &AutoExposure, step: Step) -> Self {
        let r = AutoExposure::EV_RANGE;
        // A NaN or infinite knob reads as 0, as the ops' `clamped` does.
        let fin = |v: f32, lo: f32, hi: f32| (if v.is_finite() { v } else { 0.0 }).clamp(lo, hi);
        let min_ev = fin(ae.min_ev, -r, r);
        let max_ev = fin(ae.max_ev, min_ev, r);
        let speed = |v| fin(v, 0.0, AutoExposure::MAX_SPEED);
        let low = fin(ae.low_percent, 0.0, 100.0);
        let high = fin(ae.high_percent, low, 100.0);
        Self {
            limits: [min_ev, max_ev, fin(ae.compensation, -r, r), 0.0],
            adapt: [
                speed(ae.speed_up),
                speed(ae.speed_down),
                step.dt,
                if step.snap { 1.0 } else { 0.0 },
            ],
            window: [low / 100.0, high / 100.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub dt: f32,
    pub snap: bool,
    pub cut: bool,
}

/// On game time, so a paused game holds its exposure and a capture at tick N replays.
#[derive(Default)]
pub struct Clock {
    last: Option<(HistoryKey, bool, f32)>,
}

impl Clock {
    pub(crate) fn step(&mut self, key: HistoryKey, editor: bool, time: f32) -> Step {
        let last = self.last.replace((key, editor, time));
        let cut = !matches!(last, Some((k, e, t)) if k == key && e == editor && time >= t);
        match last {
            Some((_, _, t)) if key.playing && !cut => Step {
                dt: time - t,
                snap: false,
                cut,
            },
            _ => Step {
                dt: 0.0,
                snap: true,
                cut,
            },
        }
    }

    pub fn cut(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
