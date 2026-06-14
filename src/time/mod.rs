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
    /// Seconds elapsed during the current frame.
    pub delta_time: f32,
    /// The fixed timestep used by the deterministic clock.
    pub fixed_delta_time: f32,
    /// Number of frames advanced since the clock started.
    pub frame_count: u64,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            delta_time: 0.0,
            fixed_delta_time: FIXED_DELTA_TIME,
            frame_count: 0,
        }
    }
}

impl Time {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one advanced frame of `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        self.delta_time = dt;
        self.frame_count += 1;
    }

    /// Reset the clock (e.g. on entering play mode).
    pub fn reset(&mut self) {
        self.delta_time = 0.0;
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
        assert_eq!(t.fixed_delta_time, FIXED_DELTA_TIME);
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
