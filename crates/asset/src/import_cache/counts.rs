//! What tests read of the import cache: how many lookups and parses this thread
//! made, and a fresh launch's cache for one file. Test hooks (crates/CLAUDE.md): compiled in
//! every build, since the root's tests (`physics`, `scene`) call them too. The
//! counts are kept only on a thread that asked [`lookups`] or [`parses`] once, which
//! arms it (as `ecs::world::walks` does): an unarmed lookup pays one thread-local
//! read.

use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static PARSES: Cell<usize> = const { Cell::new(0) };
    static LOOKUPS: Cell<usize> = const { Cell::new(0) };
}

/// Count one [`super::imported`] call, on an armed thread.
pub(super) fn count_lookup() {
    if ARMED.get() {
        LOOKUPS.set(LOOKUPS.get() + 1);
    }
}

/// Count one parse into the cache, on an armed thread.
pub(super) fn count_parse() {
    if ARMED.get() {
        PARSES.set(PARSES.get() + 1);
    }
}

/// How many times this thread has asked the cache for a file ([`super::imported`]),
/// served or parsed, since the first call of this function or [`parses`] on it,
/// which starts the count.
pub fn lookups() -> usize {
    ARMED.set(true);
    LOOKUPS.get()
}

/// How many files this thread has parsed into the cache since the first call of
/// this function or [`lookups`] on it, which starts the count.
pub fn parses() -> usize {
    ARMED.set(true);
    PARSES.get()
}

/// Drop `path`'s entry, as a fresh launch starts without it. Only that one: the
/// cache is one per process, so emptying it would land between another test's
/// two lookups and fail its "served from the cache" asserts.
pub fn forget(path: &std::path::Path) {
    super::with_cache(|cache| cache.remove(&path.to_string_lossy()));
}
