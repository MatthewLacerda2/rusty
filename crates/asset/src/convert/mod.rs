//! Atomicity (staging swap, lock) is this module's, never a converter's.
//!
//! stored-format: the import cache's converted `.glb` files and their fingerprints

mod blend;
mod converter;
pub(crate) mod depends;
mod fbx;
#[cfg(test)]
mod fingerprint;
pub(crate) mod gate;
pub mod glb;
mod lock;
mod record;
mod shipped;
mod stage;
mod stale;
mod texture_file;
pub mod texture_ref;

pub use converter::{converter_for, Converted, Converter, Skip, CONVERTERS};
pub use depends::{digest as source_digest, Dependency, Seen};
pub use gate::{
    conversion_allowed, convert_held, convertible_sources, mark_standalone, refresh_changed,
    refresh_sources, PlayHold,
};
pub use lock::hold as lock_cache;
pub use shipped::{cached_without_source, served_from_cache, with_cache_files};
pub use stage::{textures_dirs, textures_owner, Staging};

use super::ImportError;
use record::{record, stamp_path};
use rusty_core::{hash, paths, project};
use stale::{stale, Tool};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static CONVERSIONS: AtomicUsize = AtomicUsize::new(0);

/// Any thread's count, so a test asserting an exact value runs alone.
pub fn conversions() -> usize {
    CONVERSIONS.load(Ordering::Relaxed)
}

pub const IMPORT_CACHE_DIR: &str = "assets/generated/imported";

pub fn cached_gltf(path: &Path) -> Result<PathBuf, ImportError> {
    let converter = converter_for(path)
        .ok_or_else(|| ImportError::UnsupportedFormat(paths::extension(path)))?;
    let out = cache_location(path)?;
    if conversion_allowed() {
        refresh(converter, path, &out)?;
    } else {
        gate::serve(converter, path, &out)?;
    }
    Ok(out)
}

pub fn cache_location(path: &Path) -> Result<PathBuf, ImportError> {
    cache_location_as(path, ".glb")
}

/// Outside a project, the temp folder: a conversion never writes into a tree that is not one.
pub fn cache_location_as(path: &Path, suffix: &str) -> Result<PathBuf, ImportError> {
    Ok(project_cache_as(path, suffix)?.unwrap_or_else(|| temp_path(path, suffix)))
}

pub fn project_cache_as(path: &Path, suffix: &str) -> Result<Option<PathBuf>, ImportError> {
    let root = Path::new(".");
    Ok(project_relative(root, path)?.map(|rel| cache_path_as(root, &rel, suffix)))
}

/// A swap that fails partway leaves the cache unstamped, so the next call converts again.
pub fn refresh(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    // Never asked for a missing source: a shipped player must not start Blender.
    let present = source.is_file();
    let asked = converter
        .tool
        .filter(|_| present)
        .map(|identify| identify());
    let tool = if present {
        Tool::of(asked.as_ref())
    } else {
        Tool::unasked(converter)
    };
    if stale(converter, source, out, tool)?.is_none() {
        return Ok(());
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| write_error(dir, &e))?;
    }
    let _lock = lock::hold(out);
    // A waiter may find it already converted.
    let Some(stamp) = stale(converter, source, out, tool)? else {
        return Ok(());
    };
    let tag = record::folder_tag(out, &stamp);
    match stage::convert(converter, source, out, (&stamp, &tag)) {
        Ok(converted) => {
            CONVERSIONS.fetch_add(1, Ordering::Relaxed);
            let stored = record::Stored::written(&stamp, &tag, &converted);
            record::report(source, out, &stored);
        }
        // Unlocked, another process may have converted this source meanwhile.
        Err(e) => {
            let stored = record::current(out, &stamp).ok_or(e)?;
            record::replay(source, out, &stored);
        }
    }
    Ok(())
}

pub fn cache_path(root: &Path, rel: &Path) -> PathBuf {
    cache_path_as(root, rel, ".glb")
}

pub fn cache_path_as(root: &Path, rel: &Path, suffix: &str) -> PathBuf {
    with_suffix(&cache_root(root).join(rel), suffix)
}

fn cache_root(root: &Path) -> PathBuf {
    root.join(IMPORT_CACHE_DIR)
}

pub fn is_import_cache(path: &Path) -> bool {
    paths::to_slash(path).contains(IMPORT_CACHE_DIR)
}

/// Rebuilt from components: on Linux and macOS `\` is part of a file name.
fn project_relative(root: &Path, path: &Path) -> Result<Option<PathBuf>, ImportError> {
    if !root.join(project::PROJECT_FILE).is_file() {
        return Ok(None);
    }
    let Some(rel) = relative_to_cwd(path) else {
        return Ok(None);
    };
    let mut parts = PathBuf::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) if part.to_str().is_none() => {
                return Err(ImportError::Parse(format!(
                    "{}: a source path must be UTF-8",
                    path.display()
                )))
            }
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            _ => return Ok(None),
        }
    }
    Ok((!parts.as_os_str().is_empty()).then_some(parts))
}

fn relative_to_cwd(path: &Path) -> Option<PathBuf> {
    let rel = if path.is_relative() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir().ok()?;
        match path.strip_prefix(&cwd) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => {
                let (path, cwd) = (
                    std::fs::canonicalize(path).ok()?,
                    std::fs::canonicalize(&cwd).ok()?,
                );
                path.strip_prefix(cwd).ok()?.to_path_buf()
            }
        }
    };
    Some(rel)
}

fn temp_path(path: &Path, suffix: &str) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let digest = hash::content(absolute.to_string_lossy().as_bytes());
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    std::env::temp_dir()
        .join("rusty-import")
        .join(format!("{digest:016x}-{name}{suffix}"))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn unreadable(path: &Path) -> ImportError {
    let why = std::fs::read(path).map_or_else(|e| e.to_string(), |_| "not a file".into());
    ImportError::Parse(format!("{}: {why}", path.display()))
}

fn write_error(path: &Path, e: &std::io::Error) -> ImportError {
    ImportError::Parse(format!("writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
