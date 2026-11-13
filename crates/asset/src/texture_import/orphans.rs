//! Collecting the import cache's orphaned compressed textures (#1694): a `.bctex`
//! whose source image is gone, or whose source is a baked lightmap page (which
//! the importer no longer compresses, `asset::lightmap_page::is_baked`), is
//! deleted on refresh ([`super::refresh_all`]). Without it, every rebake left a
//! cache per page name behind: a game held 842 MB of them for 158 page names.
//!
//! Only what the refresh proves orphaned goes: a `.bctex` file under the import
//! cache's own folder for the refreshed root, whose source path answers
//! *not found*. A source that exists but cannot be read is not gone, and nothing
//! outside the import cache (the system temp folder a non-project caches in) is
//! ever walked. A model's cache is never touched: an exported project ships it
//! without its source, on purpose (`convert::served_from_cache`).

use std::path::{Path, PathBuf};

use super::CACHE_SUFFIX;
use crate::convert;
use crate::lightmap_page::is_baked;

/// Delete the orphaned compressed textures of the images under `root` (see the
/// module doc) and return how many went. Nothing outside a project, whose caches
/// live in the temp folder.
pub(super) fn collect(root: &Path) -> usize {
    let Ok(cache_dir) = convert::cache_location_as(root, "") else {
        return 0;
    };
    if !convert::is_import_cache(&cache_dir) {
        return 0;
    }
    let mut removed = 0;
    for cache in orphans(&cache_dir, root) {
        let _lock = convert::lock_cache(&cache);
        match std::fs::remove_file(&cache) {
            Ok(()) => removed += 1,
            Err(e) => log::warn!("[Import] cannot remove {}: {e}", cache.display()),
        }
    }
    if removed > 0 {
        log::info!("[Import] removed {removed} orphaned compressed texture(s)");
    }
    removed
}

/// The `.bctex` files under `cache_dir` (the cache of the images under
/// `sources`) whose source is gone or is a baked lightmap page, sorted.
pub(super) fn orphans(cache_dir: &Path, sources: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(cache_dir, &mut |cache| {
        let Ok(rel) = cache.strip_prefix(cache_dir) else {
            return;
        };
        let Some(name) = rel.to_str().and_then(|r| r.strip_suffix(CACHE_SUFFIX)) else {
            return;
        };
        let source = sources.join(name);
        if is_baked(&source) || is_gone(&source) {
            found.push(cache.to_path_buf());
        }
    });
    found.sort();
    found
}

/// Whether nothing is at `path`: the OS says *not found*. Any other failure (a
/// permission, an I/O error) is not proof it is gone.
fn is_gone(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

/// Visit every file under `dir`, not following symlinked folders.
fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => walk(&path, visit),
            Ok(kind) if kind.is_file() => visit(&path),
            _ => {}
        }
    }
}
