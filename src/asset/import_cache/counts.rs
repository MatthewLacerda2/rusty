//! What tests read of the import cache: how many parses this thread made, and a
//! fresh launch's cache for one file.

thread_local!(pub(super) static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) });

/// How many files this thread has parsed into the cache.
pub fn parses() -> usize {
    PARSES.get()
}

/// Drop `path`'s entry, as a fresh launch starts without it. Only that one: the
/// cache is one per process, so emptying it would land between another test's
/// two lookups and fail its "served from the cache" asserts.
pub fn forget(path: &std::path::Path) {
    super::with_cache(|cache| cache.remove(&path.to_string_lossy()));
}
