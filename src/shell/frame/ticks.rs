//! src/shell/frame/ticks.rs — the fixed-tick accumulator (#1877).
//!
//! A running game advances on the fixed tick, whatever the display does: each frame
//! hands its wall-clock seconds to [`FixedTicks::owe`], which says how many whole
//! [`FIXED_DELTA_TIME`] ticks are now owed and keeps the remainder for the next
//! frame (Unity's `Time.fixedDeltaTime` loop). A frame adds at most
//! [`MAXIMUM_DELTA_TIME`] (Unity's `Time.maximumDeltaTime`, 1/3 s), so a hitch
//! costs at most 20 ticks and never spirals into frames that take longer still.
//!
//! The time added is **unscaled**: `Time.timeScale` scales each tick's `deltaTime`
//! instead, as the headless harness and `Time.Step` always have, so a windowed run
//! and the harness agree tick for tick at every scale, and scripts still tick at
//! scale 0 (a pause menu's `Update` keeps running, `docs/ui.md`).

use crate::time::{FIXED_DELTA_TIME, MAXIMUM_DELTA_TIME};

/// How close to a whole tick counts as one, in ticks.
const TICK_SLACK: f64 = 1e-6;

/// Wall-clock seconds handed in and not yet ticked. `f64` so a long session's
/// remainder never drifts off the tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FixedTicks {
    owed: f64,
}

impl FixedTicks {
    /// Add a frame of `raw` wall-clock seconds (negative or NaN as zero, at most
    /// [`MAXIMUM_DELTA_TIME`]) and take the whole ticks now owed.
    pub fn owe(&mut self, raw: f32) -> u32 {
        let frame = if raw > 0.0 {
            raw.min(MAXIMUM_DELTA_TIME)
        } else {
            0.0
        }; // NaN too
        self.owed += f64::from(frame);
        let tick = f64::from(FIXED_DELTA_TIME);
        // The slack absorbs f32 rounding (1/3 s over 1/60 s is 19.9999999 in f32
        // terms): a remainder within a millionth of a tick is a whole tick.
        let whole = (self.owed / tick + TICK_SLACK).floor();
        self.owed = (self.owed - whole * tick).max(0.0);
        whole as u32
    }

    /// How far the remainder is into the next tick, `0..=1`: the blend a frame is
    /// drawn at between the last two ticks (`render::interpolate`).
    pub fn alpha(&self) -> f32 {
        (self.owed / f64::from(FIXED_DELTA_TIME)).clamp(0.0, 1.0) as f32
    }

    /// Drop the remainder: the sim stopped running on the tick (edit mode, a pause,
    /// a Play transition), and the time since is not owed to it.
    pub fn clear(&mut self) {
        self.owed = 0.0;
    }
}

#[cfg(test)]
#[path = "ticks_tests.rs"]
mod ticks_tests;
