//! The two classes of a `Debug.Stats` key: a wall-clock timing, which differs run
//! to run, and a count, which a deterministic run repeats. They cover every key
//! and never overlap.

/// Whether `key` names a wall-clock timing (non-deterministic) rather than a count:
/// a key ending in `_ms` or `_us` (`render_extract_us`, `shadow_gather_us`), or the
/// window's frame rate, `fps`.
pub fn is_timing(key: &str) -> bool {
    key.ends_with("_ms") || key.ends_with("_us") || key == "fps"
}

/// Whether `key` names a count (`draw_calls`, `changes_<channel>`, a 0/1 flag):
/// every key [`is_timing`] does not, so a new counter is one the moment it is
/// recorded, and the bench reports it (`dev::bench::sample`). A key no average
/// means anything for (an id, a mode) would be a third class here; none is today.
pub fn is_counter(key: &str) -> bool {
    !is_timing(key)
}
