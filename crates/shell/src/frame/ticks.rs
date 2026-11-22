use rusty_time::{FIXED_DELTA_TIME, MAXIMUM_DELTA_TIME};

/// How close to a whole tick counts as one, in ticks: f32 rounding makes 1/3 s over 1/60 s
/// 19.9999996 ticks.
const TICK_SLACK: f64 = 1e-6;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FixedTicks {
    /// `f64` so a long session's remainder never drifts off the tick.
    owed: f64,
    restart: bool,
}

impl FixedTicks {
    /// Add a frame of `raw` wall-clock seconds (negative or NaN as zero, at most
    /// [`MAXIMUM_DELTA_TIME`]) and take the whole ticks now owed.
    pub fn owe(&mut self, raw: f32) -> u32 {
        if std::mem::take(&mut self.restart) {
            return 0;
        }
        let frame = if raw > 0.0 {
            raw.min(MAXIMUM_DELTA_TIME)
        } else {
            0.0
        };
        self.owed += f64::from(frame);
        let tick = f64::from(FIXED_DELTA_TIME);
        let whole = (self.owed / tick + TICK_SLACK).floor();
        self.owed = (self.owed - whole * tick).max(0.0);
        whole as u32
    }

    /// How far the remainder is into the next tick, `0..=1`: the blend a frame is
    /// drawn at between the last two ticks (`render::interpolate`).
    pub fn alpha(&self) -> f32 {
        (self.owed / f64::from(FIXED_DELTA_TIME)).clamp(0.0, 1.0) as f32
    }

    /// Drop the remainder and the next frame's time: the sim was off the tick (edit mode, a
    /// pause, a Play transition, a scene load), and that time is not owed to it.
    pub fn clear(&mut self) {
        (self.owed, self.restart) = (0.0, true);
    }
}

#[cfg(test)]
#[path = "ticks_tests.rs"]
mod ticks_tests;
