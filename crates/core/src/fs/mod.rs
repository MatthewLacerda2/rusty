//! A plain write truncates first, so a concurrent reader sees an empty or torn file.
//! [`write_atomic`] writes beside the target and renames over it; [`write_cache`] skips
//! the syncs for a cache the engine re-makes.

pub mod freshness;
pub mod written;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const TEMP_SUFFIX: &str = "tmp";

/// Syncs the temp file before the rename, which can reach the disk before its data.
/// The rename replaces the file's folder entry, so it needs the folder writable,
/// and a file read-only or linked there becomes a plain file of the default mode.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace(path, bytes, write_synced)?;
    sync_folder(path);
    Ok(())
}

pub fn write_cache(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace(path, bytes, |temp, bytes| std::fs::write(temp, bytes))
}

fn replace(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let temp = unique_sibling(path, TEMP_SUFFIX);
    let result = write(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// `sync_all` is `F_FULLFSYNC` on macOS, flushing the drive's cache too.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// A failure is ignored: some file systems refuse to sync a folder.
pub fn sync_folder(path: &Path) {
    let folder = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    if let Ok(dir) = std::fs::File::open(folder) {
        let _ = dir.sync_all();
    }
}

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
