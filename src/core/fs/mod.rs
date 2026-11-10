//! src/core/fs/ — writing a file so no reader ever sees half of it (#1129).
//!
//! A plain write truncates first, so a reader in another process (or thread) at
//! that moment sees an empty or torn file. [`write_atomic`] writes beside the
//! target and renames over it: a reader gets the whole old file or the whole new
//! one. It is the one copy: the import sidecar, the import cache's stamp, the seed
//! manifest, the authored UI widgets and imported audio all write through it, and
//! the `atomic-write` rule (`make rules`) flags a `std::fs::rename` anywhere else.
//! [`written`] is the log of asset files the engine wrote, which path-keyed caches
//! drain to drop what went stale (#1207), each through a [`freshness::Freshness`]
//! that also keeps the files it could not read (#1241).

pub mod freshness;
mod retry;
pub mod written;

pub use retry::retry_transient;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The suffix of [`write_atomic`]'s temp file, `<path>.<pid>.<n>.tmp`: what a
/// crash leaves behind, for [`is_unique_sibling`] to spot.
pub const TEMP_SUFFIX: &str = "tmp";

/// Write `bytes` at `path` through a temp file beside it, renamed into place. The
/// temp file is synced to the disk before the rename, and the folder after it (on
/// Unix), so a power cut leaves the old file or the new one, never an empty one (a
/// rename can reach the disk before the data it names). The temp file is removed
/// when the write or the rename fails, so a failure leaves no litter. The parent
/// directory must exist.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = unique_sibling(path, TEMP_SUFFIX);
    // Windows refuses the rename for a moment while a reader holds the target.
    let result =
        write_synced(&temp, bytes).and_then(|()| retry_transient(|| std::fs::rename(&temp, path)));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return result;
    }
    sync_folder(path);
    Ok(())
}

/// Write `bytes` to a new file at `path` and sync it (`F_FULLFSYNC` on macOS, where
/// a plain `fsync` stops at the drive's cache).
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Sync the folder holding `path`, so a rename into it is on the disk. Unix only:
/// Windows cannot open a folder as a file, and its rename is journaled. A failure
/// is ignored (some file systems refuse to sync a folder): the file is in place.
fn sync_folder(path: &Path) {
    #[cfg(unix)]
    {
        let folder = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        if let Ok(dir) = std::fs::File::open(folder) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// `<path>.<pid>.<n>.<suffix>`: a name beside `path` no other call, in this
/// process or another, is handed at the same time.
pub fn unique_sibling(path: &Path, suffix: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{}.{n}.{suffix}", std::process::id()));
    path.with_file_name(name)
}

/// True when `name` is one [`unique_sibling`] made of the file `of` with `suffix`,
/// in any process: what a leftover from a crashed write looks like.
pub fn is_unique_sibling(name: &str, of: &str, suffix: &str) -> bool {
    let middle = name
        .strip_prefix(of)
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(suffix))
        .and_then(|rest| rest.strip_suffix('.'));
    middle
        .and_then(|m| m.split_once('.'))
        .is_some_and(|(pid, n)| is_number(pid) && is_number(n))
}

/// True when `name` is a [`write_atomic`] temp file of any target, in any process:
/// a name that is never an asset, only a write in flight (or a crash's leftover).
pub fn is_temp_name(name: &str) -> bool {
    let mut parts = name.rsplitn(4, '.');
    parts.next() == Some(TEMP_SUFFIX)
        && parts.next().is_some_and(is_number)
        && parts.next().is_some_and(is_number)
        && parts.next().is_some_and(|of| !of.is_empty())
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests;
