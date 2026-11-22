//! The two classes of a `Debug.Stats` key: a wall-clock timing, which differs run
//! to run, and a count, which a deterministic run repeats. They cover every key
//! and never overlap.

pub fn is_timing(key: &str) -> bool {
    key.ends_with("_ms") || key.ends_with("_us") || key == "fps"
}

pub fn is_counter(key: &str) -> bool {
    !is_timing(key)
}
