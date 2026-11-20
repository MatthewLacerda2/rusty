//! Collecting the import cache's orphaned compressed textures (#1694): a `.bctex`
//! whose source image is gone, or whose source the importer no longer compresses
//! (a baked lightmap page, `asset::lightmap_page::is_baked`, or a normal map kept
//! at 16 bits, `texture_import::is_deep_normal`), is deleted on refresh
//! ([`super::refresh_all`]). Without it, every rebake left a cache per page name
//! behind: a game held 842 MB of them for 158 page names.
//!
//! Only what the refresh proves orphaned goes: a `.bctex` file under the import
//! cache's own folder for the refreshed root, whose source path answers *not
//! found*, is a baked page by its path, or is a `NormalMap` (by its sidecar) whose
//! copy is not current and whose file's header says 16 bits. A current copy is
//! never one of those, so it costs a `stat` and the cache's head, never the
//! source's header. A texture extracted beside a model's cache has its copy beside
//! that cache (`super::extracted_cache`, #2365), and is gone once the textures
//! folder the model's glTF names holds no image of its name: the model gone, or
//! re-converted without it. A source that exists but cannot be read is not gone, and nothing
//! outside the import cache (the system temp folder a non-project caches in) is
//! ever walked. A model's cache is never touched: an exported project ships it
//! without its source, on purpose (`convert::served_from_cache`).

use std::path::{Path, PathBuf};

use super::CACHE_SUFFIX;
use crate::convert::texture_ref::extracted;
use crate::convert::{self, Seen};
use crate::lightmap_page::is_baked;
use crate::sidecar::TextureType;

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
/// `sources`) whose source is gone, is a baked lightmap page or is a normal map
/// kept at 16 bits, sorted.
pub(super) fn orphans(cache_dir: &Path, sources: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(cache_dir, &mut |cache| {
        let Ok(rel) = cache.strip_prefix(cache_dir) else {
            return;
        };
        let Some(name) = rel.to_str().and_then(|r| r.strip_suffix(CACHE_SUFFIX)) else {
            return;
        };
        let source = match extracted_source(cache) {
            Some(extracted) => extracted,
            None => sources.join(name),
        };
        let deep = || deep_orphan(cache, &source);
        if is_baked(&source) || is_gone(&source) || deep() {
            found.push(cache.to_path_buf());
        }
    });
    found.sort();
    found
}

/// The image the compressed copy at `cache` was made from when it is a texture
/// extracted beside a model's cache (`<glb>.bctex/<name>.bctex`,
/// `super::extracted_cache`): the image of that name in the textures folder the
/// model's glTF names now, or where it would lie when the glTF names none (so it
/// is gone). `None` for any other copy.
fn extracted_source(cache: &Path) -> Option<PathBuf> {
    let folder = cache.parent()?;
    let glb = folder.to_str()?.strip_suffix(CACHE_SUFFIX)?;
    if !glb.ends_with(".glb") {
        return None;
    }
    let glb = Path::new(glb);
    let name = cache.file_name()?.to_str()?.strip_suffix(CACHE_SUFFIX)?;
    let named = extracted::images(glb)
        .into_iter()
        .find(|image| image.file_name().is_some_and(|n| n == name));
    Some(named.unwrap_or_else(|| folder.join(name)))
}

/// Whether `cache`, the copy of the existing image at `source`, is a 16-bit normal
/// map's: a `NormalMap` whose copy is stale ([`super::current`] would refuse it)
/// and whose header says more than 8 bits a channel (the module doc).
fn deep_orphan(cache: &Path, source: &Path) -> bool {
    let settings = crate::sidecar::texture(source);
    if settings.texture_type != TextureType::NormalMap {
        return false;
    }
    let current = match convert::source_digest(source) {
        Seen::Read(digest) => {
            super::cache::is_current(cache, super::cache::stamp(digest, &settings))
        }
        _ => false,
    };
    !current && super::is_deep_normal(source, &settings)
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
