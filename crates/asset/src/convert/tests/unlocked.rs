//! A filesystem without locks is warned of once per process (#1166, #1197): the
//! first failed `lock::hold` warns, the ones after it stay quiet. Relies on
//! nextest's process per test, as `WARNED` is per process.

use super::super::lock;
use std::path::Path;

/// How many logged warnings name the lock of `out`.
fn warnings_of(out: &Path) -> usize {
    let line = format!("[Asset] cannot lock the import cache {} (", out.display());
    rusty_test_support::logs::take()
        .iter()
        .filter(|m| m.starts_with(&line))
        .count()
}

#[test]
fn only_the_first_lock_that_cannot_be_taken_warns() {
    rusty_test_support::logs::init();
    // No folder for the cache: `hold` cannot name its lock.
    let out = rusty_test_support::temp::dir()
        .join("absent")
        .join("a.src.glb");
    assert!(lock::hold(&out).is_none());
    assert_eq!(warnings_of(&out), 1, "the first failure warns");
    assert!(lock::hold(&out).is_none());
    assert_eq!(warnings_of(&out), 0, "a second failure stays quiet");
}
