//! A cache without its source (#1267): an exported project ships the import
//! cache and leaves its converted sources behind, so the cache stands in for
//! them. The one rule for when it does, and the files a cache is made of.

use super::{cache_location, converter_for, lock, stage, stamp_path};
use std::path::{Path, PathBuf};

/// Whether the cache at `out` stands in for its source: the source was not read
/// (`read` false: it is absent, or not a readable file) and the cache is there.
/// The one rule `stale` serves a cache by and `Debug.Preview` finds an asset by.
pub fn served_from_cache(read: bool, out: &Path) -> bool {
    !read && out.is_file()
}

/// Whether `path` names a converted source that is not there, whose cache stands
/// in for it.
pub fn cached_without_source(path: &Path) -> bool {
    converter_for(path).is_some()
        && cache_location(path).is_ok_and(|out| served_from_cache(path.is_file(), &out))
}

/// Run `f` on the files of the cache at `out` as it stands, sorted: the glTF, its
/// stamp and every file of its textures folders. The cache's lock (`lock`) is held
/// meanwhile, so no conversion swaps it under `f`. No files when there is no cache.
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
