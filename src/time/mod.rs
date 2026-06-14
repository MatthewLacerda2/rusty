//! src/time/mod.rs — Time resource + fixed clock
//!
//! Holds the per-frame `delta_time`, the fixed-timestep `fixed_delta_time` the
//! headless harness drives, and a monotonic `frame_count`. The simulation advances
//! it once per tick; Lua reads it through the `Time` namespace (Unity: `Time`).
//!
//! Allowed deps: none.

/// The canonical fixed timestep (60 Hz). Matches `dev::harness::FIXED_DT`; kept
/// here too so non-dev builds have the value without pulling in the dev tree.
pub const FIXED_DELTA_TIME: f32 = 1.0 / 60.0;

/// Frame-clock resource. One instance lives in the `GameWorld`, shared into the
/// script runtime so scripts can query time deterministically.
#[derive(Clone, Debug)]
pub struct Time {
    /// Seconds elapsed during the current frame, scaled by `time_scale`.
    pub delta_time: f32,
    /// Seconds elapsed during the current frame, ignoring `time_scale`. Used by
    /// pause menus / UI that keep animating while the sim is paused.
    pub unscaled_delta_time: f32,
    /// The fixed timestep used by the deterministic clock. Always unscaled — it's
    /// the engine's fixed step.
    pub fixed_delta_time: f32,
    /// Number of frames advanced since the clock started.
    pub frame_count: u64,
    /// Global simulation time scale (Unity: `Time.timeScale`). `1.0` = realtime,
    /// `0.0` = paused, `0.5` = slow-mo, `2.0` = fast. Never negative.
    pub time_scale: f32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            delta_time: 0.0,
            unscaled_delta_time: 0.0,
            fixed_delta_time: FIXED_DELTA_TIME,
            frame_count: 0,
            time_scale: 1.0,
        }
    }
}

impl Time {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one advanced frame of `raw_dt` seconds. `delta_time` is scaled by
    /// `time_scale`; `unscaled_delta_time` keeps the raw value.
    pub fn advance(&mut self, raw_dt: f32) {
        self.unscaled_delta_time = raw_dt;
        self.delta_time = raw_dt * self.time_scale;
        self.frame_count += 1;
    }

    /// Set the global time scale, clamping negatives to `0.0`.
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale.max(0.0);
    }

    /// Reset the clock (e.g. on entering play mode). `time_scale` persists — it's
    /// deterministic game state, not a per-frame value.
    pub fn reset(&mut self) {
        self.delta_time = 0.0;
        self.unscaled_delta_time = 0.0;
        self.frame_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_counts_frames_and_tracks_delta() {
        let mut t = Time::new();
        t.advance(0.5);
        t.advance(0.25);
        assert_eq!(t.frame_count, 2);
        assert_eq!(t.delta_time, 0.25);
        assert_eq!(t.unscaled_delta_time, 0.25);
        assert_eq!(t.fixed_delta_time, FIXED_DELTA_TIME);
    }

    #[test]
    fn time_scale_scales_delta_but_not_unscaled() {
        let mut t = Time::new();
        assert_eq!(t.time_scale, 1.0);

        // Pause: scaled delta freezes, raw delta keeps flowing.
        t.set_time_scale(0.0);
        t.advance(0.5);
        assert_eq!(t.delta_time, 0.0);
        assert_eq!(t.unscaled_delta_time, 0.5);

        // Slow-mo: scaled delta is halved.
        t.set_time_scale(0.5);
        t.advance(0.5);
        assert_eq!(t.delta_time, 0.25);
        assert_eq!(t.unscaled_delta_time, 0.5);

        // fixed step never scales.
        assert_eq!(t.fixed_delta_time, FIXED_DELTA_TIME);
    }

    #[test]
    fn set_time_scale_clamps_negatives() {
        let mut t = Time::new();
        t.set_time_scale(-2.0);
        assert_eq!(t.time_scale, 0.0);
    }

    #[test]
    fn reset_keeps_time_scale() {
        let mut t = Time::new();
        t.set_time_scale(0.5);
        t.reset();
        assert_eq!(t.time_scale, 0.5);
    }

    #[test]
    fn reset_clears_frames() {
        let mut t = Time::new();
        t.advance(0.1);
        t.reset();
        assert_eq!(t.frame_count, 0);
        assert_eq!(t.delta_time, 0.0);
    }
}
