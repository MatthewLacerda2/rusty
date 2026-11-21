//! The dev layer's clock as milliseconds: the stats' spans and the bench's
//! `wall_ms` read it, the wall clock or a test's ([`verbs`](super::verbs)).

use std::time::Instant;

/// Now, on the dev layer's clock: what [`ms_since`] measures against.
pub fn now() -> Instant {
    super::verbs::now()
}

/// The milliseconds from `start` to now, on the dev layer's clock.
pub fn ms_since(start: Instant) -> f64 {
    ms_between(start, now())
}

/// The milliseconds from `start` to `end`, zero when `end` is the earlier.
pub fn ms_between(start: Instant, end: Instant) -> f64 {
    end.saturating_duration_since(start).as_secs_f64() * 1000.0
}
