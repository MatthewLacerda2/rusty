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
    /// Loop-level pause flag (issue #283). When `true`, the windowed runtime stops
    /// advancing the sim with wall-clock dt — the frame loop reads this **before**
    /// it ticks and skips the real-time advance entirely (rendering still runs each
    /// frame). This is a hard halt distinct from `time_scale`: `time_scale` scales
    /// `delta_time` *while the sim runs*, whereas `paused` decides *whether the loop
    /// runs the sim at all*. They compose — a paused world stepped via `pending_steps`
    /// still scales its fixed dt by `time_scale`.
    pub paused: bool,
    /// Queued fixed-dt steps to pump while paused (issue #283). `Step(n)` adds `n`;
    /// the windowed loop drains them one `FIXED_DELTA_TIME` tick at a time,
    /// decrementing, then halts again. Only meaningful while `paused`.
    pub pending_steps: u32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            delta_time: 0.0,
            unscaled_delta_time: 0.0,
            fixed_delta_time: FIXED_DELTA_TIME,
            frame_count: 0,
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

    /// Loop-level pause (issue #283): freeze the windowed real-time advance while
    /// keeping all world state. Distinct from `set_time_scale(0)` — see `paused`.
    /// Idempotent.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume normal real-time play (issue #283) from the *current* stepped-to state
    /// (NOT the pre-play snapshot — that's Stop). Clears any queued steps so a resume
    /// never silently leaks a leftover step into real-time play. Idempotent.
    pub fn resume(&mut self) {
        self.paused = false;
        self.pending_steps = 0;
    }

    /// Queue `n` fixed-dt steps to pump while paused (issue #283). Steps only make
    /// sense paused — the windowed loop ignores `pending_steps` when not `paused` —
    /// so this is a no-op (and a logged hint at the API layer) when not paused. Adds
    /// to any steps still pending so rapid `Step` calls accumulate.
    pub fn request_steps(&mut self, n: u32) {
        if self.paused {
            self.pending_steps = self.pending_steps.saturating_add(n);
        }
    }

    /// Consume one queued step if any remain (issue #283): returns `true` and
    /// decrements when a step was pending, `false` when the queue is empty. The
    /// windowed loop calls this each frame while paused to drive the fixed-dt pump.
    pub fn take_pending_step(&mut self) -> bool {
        if self.pending_steps > 0 {
            self.pending_steps -= 1;
            true
        } else {
            false
        }
    }

    /// Reset the clock (e.g. on entering play mode). `time_scale`, `paused` and
    /// `pending_steps` persist — pause/step is a deliberate runtime control the
    /// player drives, not per-frame bookkeeping; a fresh Play that wants real-time
    /// starts unpaused already (default), and the editor never enters paused.
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

    // --- Pause / step / resume control state machine (issue #283) ---

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
        // Not paused: Step is a no-op (stepping is only meaningful while frozen).
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
        // Exactly three steps drain, then the pump reports empty (halted).
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
