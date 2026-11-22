//! A lock file of its own, never the source (an exporter renaming over it would hand
//! the next conversion an unlocked file), and never removed on release (a waiter
//! would lock a file no newcomer opens). Never beside the cache: that sits in
//! `assets/`, which may be versioned or shipped. Named by a digest of the canonical
//! cache path, so any spelling opens one file.

use super::IMPORT_CACHE_DIR;
use rusty_core::{hash, paths, project};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static WARNED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static UNAVAILABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Every [`hold`] `f` makes on this thread takes none.
#[cfg(test)]
pub fn unavailable<T>(f: impl FnOnce() -> T) -> T {
    UNAVAILABLE.with(|u| u.set(true));
    let result = f();
    UNAVAILABLE.with(|u| u.set(false));
    result
}

/// `None` when the filesystem has no locks: the conversion then runs unlocked.
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

pub const LOCKS_DIR: &str = "import-locks";

pub fn path(out: &Path) -> std::io::Result<PathBuf> {
    let (Some(dir), Some(name)) = (out.parent(), out.file_name()) else {
        return Err(std::io::Error::other("not a file path"));
    };
    // The folder, not the file: the cache may not exist yet.
    let canonical = std::fs::canonicalize(if dir.as_os_str().is_empty() {
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
    Ok(locks.join(format!("{:016x}.lock", hash::content(key.as_bytes()))))
}

fn project_of(cache: &Path) -> Option<&Path> {
    cache
        .ancestors()
        .skip(1)
        .find(|root| cache.starts_with(root.join(IMPORT_CACHE_DIR)))
}
