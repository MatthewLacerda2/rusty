//! What the exposure passes are told each frame (#1444): the volume's knobs, packed
//! for `postfx/exposure.wgsl`, and how far adaptation may move ([`Step`]), decided
//! by the view's [`Clock`].

use crate::components::AutoExposure;
use crate::render::motion::HistoryKey;

/// CPU mirror of `Params` in `postfx/exposure.wgsl`.
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
    /// Pack `ae` for a frame that adapts by `step`. Held to the ops' ranges, since a
    /// loaded file bypasses them.
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

/// How far one frame's adaptation may move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Game seconds since the view last adapted.
    pub dt: f32,
    /// Go straight to the metered target, without easing.
    pub snap: bool,
    /// The frame does not follow the last one this clock adapted (a cut, not just a
    /// snap outside Play): what was read back before it no longer applies.
    pub cut: bool,
}

/// One view's adaptation clock: the game time it last adapted at, and what that
/// was valid for. Adaptation runs on game time (`Scene::shader_time`), so a paused
/// game (time scale 0) holds its exposure and a capture at tick N replays exactly.
#[derive(Default)]
pub struct Clock {
    /// What the last adapted frame drew (its key, whether the editor's Scene view
    /// drew it, its game time), `None` after a cut.
    last: Option<(HistoryKey, bool, f32)>,
}

impl Clock {
    /// The step for a frame of `key` at game time `time`, drawn by the editor's
    /// Scene view when `editor`. It cuts on the first frame, after [`Clock::cut`],
    /// when the scene, the base camera or the play state changed (the motion
    /// history's cuts, #1435), when the editor's Scene and Game tabs swap (they
    /// share one view, so one meter) and when time ran backwards; it snaps on a cut
    /// and outside Play (the editor shows the settled exposure); otherwise it eases
    /// by the time passed.
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

    /// Forget the last frame: the next one snaps. A resize, and a frame the view
    /// did not meter (its volume in `Fixed` mode, or none), do this.
    pub fn cut(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
