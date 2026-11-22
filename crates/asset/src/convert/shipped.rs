//! An exported project ships the import cache without its sources.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

use super::{cache_location, converter_for, lock, stage, stamp_path};
use std::path::{Path, PathBuf};

/// `read` false: the source is absent or unreadable. The one rule `stale` and
/// `Debug.Preview` use for a cache standing in for its source.
pub fn served_from_cache(read: bool, out: &Path) -> bool {
    !read && out.is_file()
}

pub fn cached_without_source(path: &Path) -> bool {
    converter_for(path).is_some()
        && cache_location(path).is_ok_and(|out| served_from_cache(path.is_file(), &out))
}

/// Sorted files; the cache's lock is held so no conversion swaps it under `f`.
pub fn with_cache_files<T>(out: &Path, f: impl FnOnce(&[PathBuf]) -> T) -> T {
    if !out.is_file() {
        return f(&[]);
    }
    let _lock = lock::hold(out);
    let mut files = vec![out.to_path_buf()];
    let stamp = stamp_path(out);
    if stamp.is_file() {
        files.push(stamp);
    }
    for dir in stage::textures_dirs(out) {
        files_under(&dir, &mut files);
    }
    files.sort();
    f(&files)
}

fn files_under(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, files);
        } else {
            files.push(path);
        }
    }
}
