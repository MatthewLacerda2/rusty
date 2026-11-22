#![deny(clippy::unwrap_used)]

/// Seconds per Play tick: 60 Hz.
pub const FIXED_DELTA_TIME: f32 = 1.0 / 60.0;

/// Seconds: the most wall clock one windowed or on-demand frame hands the sim.
pub const MAXIMUM_DELTA_TIME: f32 = 1.0 / 3.0;

#[derive(Clone, Debug)]
pub struct Time {
    pub delta_time: f32,
    pub unscaled_delta_time: f32,
    pub fixed_delta_time: f32,
    /// Number of frames advanced since the clock started.
    pub frame_count: u64,
    /// Scaled game seconds; `f64` stays sub-millisecond over hours of play.
    pub time: f64,
    pub unscaled_time: f64,
    /// Global simulation time scale (Unity: `Time.timeScale`), never negative.
    pub time_scale: f32,
    pub paused: bool,
    pub pending_steps: u32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            delta_time: 0.0,
            unscaled_delta_time: 0.0,
            fixed_delta_time: FIXED_DELTA_TIME,
            frame_count: 0,
            time: 0.0,
            unscaled_time: 0.0,
            time_scale: 1.0,
            paused: false,
            pending_steps: 0,
        }
    }
}

impl Time {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance(&mut self, raw_dt: f32) {
        self.unscaled_delta_time = raw_dt;
        self.delta_time = raw_dt * self.time_scale;
        self.frame_count += 1;
        self.time += f64::from(self.delta_time);
        self.unscaled_time += f64::from(raw_dt);
    }

    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale.max(0.0);
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.pending_steps = 0;
    }

    pub fn request_steps(&mut self, n: u32) {
        if self.paused {
            self.pending_steps = self.pending_steps.saturating_add(n);
        }
    }

    pub fn take_pending_step(&mut self) -> bool {
        if self.pending_steps > 0 {
            self.pending_steps -= 1;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.delta_time = 0.0;
        self.unscaled_delta_time = 0.0;
        self.frame_count = 0;
        self.time = 0.0;
        self.unscaled_time = 0.0;
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
        assert_eq!(t.time, 0.25);

        // The fixed step never scales.
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
        assert_eq!(t.time, 0.0);
    }

    #[test]
    fn default_is_unpaused_with_no_pending_steps() {
        let t = Time::new();
        assert!(!t.paused);
        assert_eq!(t.pending_steps, 0);
    }

    #[test]
    fn pause_sets_the_flag_and_is_idempotent() {
        let mut t = Time::new();
        t.pause();
        assert!(t.paused);
        t.pause();
        assert!(t.paused, "pause must be idempotent");
    }

    #[test]
    fn steps_only_enqueue_while_paused() {
        let mut t = Time::new();
        // Not paused: Step is a no-op.
        t.request_steps(5);
        assert_eq!(t.pending_steps, 0);
        // Paused: Step enqueues, and repeated calls accumulate.
        t.pause();
        t.request_steps(3);
        t.request_steps(2);
        assert_eq!(t.pending_steps, 5);
    }

    #[test]
    fn draining_yields_exactly_n_steps_then_halts() {
        let mut t = Time::new();
        t.pause();
        t.request_steps(3);
        assert!(t.take_pending_step());
        assert!(t.take_pending_step());
        assert!(t.take_pending_step());
        assert!(!t.take_pending_step(), "queue must be empty after N steps");
        assert_eq!(t.pending_steps, 0);
    }

    #[test]
    fn resume_clears_paused_and_pending() {
        let mut t = Time::new();
        t.pause();
        t.request_steps(4);
        t.resume();
        assert!(!t.paused, "resume clears the pause flag");
        assert_eq!(t.pending_steps, 0, "resume drops any leftover steps");
    }

    #[test]
    fn pause_and_steps_survive_a_reset() {
        // Pause/step is a deliberate runtime control, not per-frame state, so a
        // clock reset (entering play) must not silently un-pause a stepped session.
        let mut t = Time::new();
        t.pause();
        t.request_steps(2);
        t.reset();
        assert!(t.paused);
        assert_eq!(t.pending_steps, 2);
    }
}
