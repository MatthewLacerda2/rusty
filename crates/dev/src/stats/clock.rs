use std::time::Instant;

/// The dev layer's clock: the wall clock, or the driven one of a test that called
/// `verbs::drive`.
pub fn now() -> Instant {
    super::verbs::now()
}

pub fn ms_since(start: Instant) -> f64 {
    ms_between(start, now())
}

pub fn ms_between(start: Instant, end: Instant) -> f64 {
    end.saturating_duration_since(start).as_secs_f64() * 1000.0
}
