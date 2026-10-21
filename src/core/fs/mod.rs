//! src/core/fs/ — writing a file so no reader ever sees half of it (#1129).
//!
//! A plain write truncates first, so a reader in another process (or thread) at
//! that moment sees an empty or torn file. [`write_atomic`] writes beside the
//! target and renames over it: a reader gets the whole old file or the whole new
//! one. It is the one copy: the import sidecar, the import cache's stamp and the
//! seed manifest all write through it.

mod retry;

pub use retry::retry_transient;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Write `bytes` at `path` through a temp file beside it, renamed into place. The
/// temp file is removed when the write or the rename fails, so a failure leaves no
/// litter. The parent directory must exist.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = unique_sibling(path, "tmp");
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

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests;
