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
/// temp file is removed when the write or the rename fails, so a failure leaves no
/// litter. The parent directory must exist.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = unique_sibling(path, TEMP_SUFFIX);
    // Windows refuses the rename for a moment while a reader holds the target.
    let result = std::fs::write(&temp, bytes)
        .and_then(|()| retry_transient(|| std::fs::rename(&temp, path)));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
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
