//! Deletes on refresh the `.bctex` whose source is gone, is a baked lightmap page, or is a stale
//! `NormalMap` copy of a 16-bit file. Only proven orphans go: a source that exists but cannot be
//! read is not gone, nothing outside the import cache is walked, and a model's cache is never
//! touched (an export ships it without its source, `convert::served_from_cache`). An extracted
//! texture is gone once its model's glTF names no image of its name.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::CACHE_SUFFIX;
use crate::convert::texture_ref::extracted;
use crate::convert::{self, Seen};
use crate::lightmap_page::is_baked;
use crate::sidecar::TextureType;

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
            Ok(()) => {
                removed += 1;
                if let Some(folder) = extracted_folder(&cache) {
                    let _ = std::fs::remove_dir(folder);
                }
            }
            Err(e) => log::warn!("[Import] cannot remove {}: {e}", cache.display()),
        }
    }
    if removed > 0 {
        log::info!("[Import] removed {removed} orphaned compressed texture(s)");
    }
    removed
}

pub(super) fn orphans(cache_dir: &Path, sources: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut named = BTreeMap::new();
    walk(cache_dir, &mut |cache| {
        let Ok(rel) = cache.strip_prefix(cache_dir) else {
            return;
        };
        let Some(name) = rel.to_str().and_then(|r| r.strip_suffix(CACHE_SUFFIX)) else {
            return;
        };
        let source = match extracted_source(cache, &mut named) {
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

fn extracted_source(cache: &Path, named: &mut Named) -> Option<PathBuf> {
    let folder = extracted_folder(cache)?;
    let name = cache.file_name()?.to_str()?.strip_suffix(CACHE_SUFFIX)?;
    let images = named.entry(folder.to_path_buf()).or_insert_with(|| {
        let glb = folder
            .as_os_str()
            .to_str()
            .and_then(|f| f.strip_suffix(CACHE_SUFFIX));
        let images = glb
            .map(|g| extracted::images(Path::new(g)))
            .unwrap_or_default();
        let by_name = |i: PathBuf| Some((i.file_name()?.to_str()?.to_string(), i));
        images.into_iter().filter_map(by_name).collect()
    });
    Some(
        images
            .get(name)
            .cloned()
            .unwrap_or_else(|| folder.join(name)),
    )
}

type Named = BTreeMap<PathBuf, BTreeMap<String, PathBuf>>;

fn extracted_folder(cache: &Path) -> Option<&Path> {
    let folder = cache.parent()?;
    let glb = folder.to_str()?.strip_suffix(CACHE_SUFFIX)?;
    glb.ends_with(".glb").then_some(folder)
}

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

/// Only *not found* proves it is gone; a permission or I/O error does not.
fn is_gone(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

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
