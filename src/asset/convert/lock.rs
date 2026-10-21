//! The lock that keeps two conversions of one cache from interleaving (#1166).
//!
//! A `<cache>.lock` file beside the cache, never the source: Windows' locks are
//! mandatory, so a lock on the source refuses the converter's own read of it, and
//! an exporter that saves by renaming a new file over the source would hand the
//! next conversion an unlocked file. The lock file stays when released: removing
//! it would let a waiter lock a file no newcomer opens.

use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once a missing lock has been warned of: once per process is enough.
static WARNED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static UNAVAILABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f` as on a filesystem with no locks: every [`hold`] it makes on this
/// thread takes none.
#[cfg(test)]
pub fn unavailable<T>(f: impl FnOnce() -> T) -> T {
    UNAVAILABLE.with(|u| u.set(true));
    let result = f();
    UNAVAILABLE.with(|u| u.set(false));
    result
}

/// Take the lock of the cache at `out`, waiting for whoever holds it; released
/// when the returned file drops. `None` when the filesystem has no locks: the
/// conversion then runs unlocked, which `stage` keeps whole in most
/// interleavings, and that is warned of once.
pub fn hold(out: &Path) -> Option<File> {
    #[cfg(test)]
    if UNAVAILABLE.with(std::cell::Cell::get) {
        return None;
    }
    let path = super::with_suffix(out, ".lock");
    let locked = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .and_then(|file| file.lock().map(|()| file));
    match locked {
        Ok(file) => Some(file),
        Err(e) => {
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "[Asset] cannot lock {} ({e}): imports convert unlocked",
                    path.display()
                );
            }
            None
        }
    }
}
