//! Test hooks, compiled in every build because the root's tests call them. Counts
//! are kept only on a thread that asked [`lookups`] or [`parses`] once (arming it).

use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static PARSES: Cell<usize> = const { Cell::new(0) };
    static LOOKUPS: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn count_lookup() {
    if ARMED.get() {
        LOOKUPS.set(LOOKUPS.get() + 1);
    }
}

pub(super) fn count_parse() {
    if ARMED.get() {
        PARSES.set(PARSES.get() + 1);
    }
}

/// Lookups on this thread since the first call of this or [`parses`].
pub fn lookups() -> usize {
    ARMED.set(true);
    LOOKUPS.get()
}

/// Parses on this thread since the first call of this or [`lookups`].
pub fn parses() -> usize {
    ARMED.set(true);
    PARSES.get()
}

/// Drops only `path`: the cache is per process, so emptying it would break
/// another test's "served from the cache" asserts.
pub fn forget(path: &std::path::Path) {
    super::with_cache(|cache| cache.remove(&path.to_string_lossy()));
}
