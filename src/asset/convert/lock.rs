//! The lock that keeps two conversions of one cache from interleaving (#1166).
//!
//! A lock file of its own, never the source: Windows' locks are mandatory, so a
//! lock on the source refuses the converter's own read of it, and an exporter that
//! saves by renaming a new file over the source would hand the next conversion an
//! unlocked file. The lock file stays when released: removing it would let a
//! waiter lock a file no newcomer opens.
//!
//! **Where it lives** (#1190): never beside the cache, which sits in `assets/`
//! and may be versioned or shipped (#866). A project's cache locks in its
//! machine-local, gitignored [`project::CACHE_DIR`] (Unity's `Library/`), in
//! [`LOCKS_DIR`];
//! any other cache (the temp folder's, a test's) in the temp folder. The file is
//! named by a digest of the cache's canonical path (project-relative inside a
//! project), so every process naming one cache, however it spells the path,
//! opens one file. Two caches whose digests collide only wait on each other.

use super::IMPORT_CACHE_DIR;
use crate::core::{hash, paths, project};
use std::fs::File;
use std::path::{Path, PathBuf};
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
    let locked = path(out).and_then(|path| {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        file.lock().map(|()| file)
    });
    match locked {
        Ok(file) => Some(file),
        Err(e) => {
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "[Asset] cannot lock the import cache {} ({e}): imports convert unlocked",
                    out.display()
                );
            }
            None
        }
    }
}

/// The folder of import locks, inside a project's [`project::CACHE_DIR`] or the
/// temp folder's `rusty-import/`.
pub const LOCKS_DIR: &str = "import-locks";

/// The lock file of the cache at `out`, whose folder exists: in the project's
/// [`LOCKS_DIR`] when `out` is in a project's import cache, else in the temp
/// folder (see the module doc).
pub fn path(out: &Path) -> std::io::Result<PathBuf> {
    let (Some(dir), Some(name)) = (out.parent(), out.file_name()) else {
        return Err(std::io::Error::other("not a file path"));
    };
    // The folder, not the file: the cache may not exist yet.
    let canonical = paths::canonicalize(if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    })?
    .join(name);
    let (locks, key) = match project_of(&canonical) {
        Some(root) => (
            root.join(project::CACHE_DIR).join(LOCKS_DIR),
            canonical.strip_prefix(root).unwrap_or(&canonical),
        ),
        None => (
            std::env::temp_dir().join("rusty-import").join(LOCKS_DIR),
            canonical.as_path(),
        ),
    };
    let key = paths::to_slash(key);
    Ok(locks.join(format!("{:016x}.lock", hash::fnv1a64(key.as_bytes()))))
}

/// The project root whose import cache holds `cache`, if one does.
fn project_of(cache: &Path) -> Option<&Path> {
    cache
        .ancestors()
        .skip(1)
        .find(|root| cache.starts_with(root.join(crate::core::paths::native(IMPORT_CACHE_DIR))))
}
