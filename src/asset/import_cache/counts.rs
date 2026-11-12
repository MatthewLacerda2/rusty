//! What tests read of the import cache: how many parses this thread made, and a
//! fresh launch's empty cache.

thread_local!(pub(super) static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) });

/// How many files this thread has parsed into the cache.
pub fn parses() -> usize {
    PARSES.get()
}

/// Drop every entry, as a fresh launch starts.
pub fn forget_all() {
    super::with_cache(super::Cache::clear);
}
