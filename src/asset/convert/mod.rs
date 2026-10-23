//! src/asset/convert/ — sources converted to glTF at import (#1024).
//!
//! The engine's loaders read glTF (and `.obj`); every other model format is turned
//! into a cached `.glb` when it is imported, the way Unity read only FBX and had
//! Blender export one from a `.blend` (CLAUDE.md, *One format inside*). The source
//! stays in the project and remains what scenes reference (`soldier.fbx::Body`);
//! [`cached_gltf`] hands the importer the glTF to read in its place, re-making it
//! when the source changed.
//!
//! **Where the cache lives.** Inside an open project (a working directory holding
//! `project.rusty`), at [`IMPORT_CACHE_DIR`]`/<project-relative source path>.glb`,
//! beside the project's other generated assets: whether that folder is versioned
//! or ignored (#866) the cache follows, and a shipped game whose sources were
//! stripped still finds it. Anywhere else (a test, a file outside the project) the
//! glTF is written to the system temp folder, so a conversion never writes into a
//! tree that is not a project. Either way the cache is a function of the whole
//! source path, so two sources never share one (#1167).
//!
//! **When it is re-made.** A `<cache>.glb.source` stamp beside each cached file
//! records the converter, its [`Converter::version`], a digest of the source's
//! bytes, the version of the program it runs ([`Converter::tool`]: Blender's, for a
//! `.blend`), and every external file the conversion looked at (an FBX's on-disk
//! texture, a `.blend`'s linked library), with a digest or as absent (#1189); any
//! difference re-converts. With the program missing, a cache of the unchanged
//! source stands, made by whichever version of it; Play never asks the program at
//! all (`gate::serve`), and a refresh asks it once (`stale`).
//! Content, not timestamps: a fresh clone gives every file a new mtime. A source's
//! digest is remembered by its length and mtime (`depends`), so a Play that looks
//! at every source reads only the ones that changed. Files a conversion writes for the glTF to
//! reference (FBX's embedded textures, #1082) live in a
//! `<cache>.glb.<tag>.textures/` folder beside it and are re-made with it; the tag,
//! recorded in the stamp file, digests the stamp and how the files the last
//! conversion looked at read now (`record::folder_tag`).
//!
//! **How it is re-made.** A converter writes plain files into a fresh staging
//! folder, and `stage` swaps the set into place, so a reader in another process
//! finds the set whole when it opens the `.glb` (#1129). A lock file in the
//! project's machine-local `cache/`, never in `assets/` (`lock`, #1190), keeps two
//! conversions of one cache from interleaving; a cache already current is read
//! without taking it (#1166). Atomicity is this module's, never a
//! converter's.
//!
//! **What a conversion left out.** A converter never logs: it returns what it
//! skipped on purpose (an unsupported texture format, a take that would not
//! bake) in [`Converted`], and fails on anything else, so a cache missing a part
//! by accident is never stamped current (#1168). [`refresh`] warns of each skip,
//! and stores them under the stamp so a cached load in a fresh process warns
//! again, once (`record`), as Unity shows a model's import warnings on every
//! reimport.
//!
//! **When it is not re-made.** Play never converts (`gate`, #1253): entering it
//! brings every source under the project's `assets/` up to date first, then it
//! reads the cache as it stands, and a source changed meanwhile converts at the
//! first edit frame after Stop. The standalone player refreshes and holds the
//! same way.
//!
//! **What a scene saves** for an extracted texture is never its cache path but
//! `model.fbx#tex/<identity>`, which [`texture_ref`] resolves (#1130).
//!
//! **Adding a converter** is one row in [`CONVERTERS`]: an extension and a
//! function that writes a glTF where its [`Staging`] says.

mod blend;
mod converter;
mod depends;
mod fbx;
mod gate;
pub mod glb;
mod lock;
mod record;
mod stage;
mod stale;
mod texture_file;
pub mod texture_ref;

pub use converter::{converter_for, Converted, Converter, Skip, CONVERTERS};
pub use depends::{Dependency, Seen};
pub use gate::{conversion_allowed, convert_held, mark_standalone, refresh_sources, PlayHold};
pub use stage::{textures_dirs, Staging};

use super::ImportError;
use crate::core::{hash, paths, project};
use record::{record, stamp_path};
use stale::{stale, Tool};
use std::path::{Component, Path, PathBuf};

/// Where an open project keeps converted sources, relative to its root: a folder
/// of [`project::GENERATED_DIR`].
pub const IMPORT_CACHE_DIR: &str = "assets/generated/imported";

/// The up-to-date glTF standing in for the converted source at `path`, converting
/// it first when the cache is missing or stale. Cached relative to the working
/// directory when that is a project (see the module doc). While conversion is off
/// (Play, a shipped game), the cache as it stands, and an error when there is none.
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

/// Where the cached glTF for the converted source at `path` lives, made or not.
pub fn cache_location(path: &Path) -> Result<PathBuf, ImportError> {
    let root = Path::new(".");
    Ok(match project_relative(root, path)? {
        Some(rel) => cache_path(root, &rel),
        None => temp_path(path),
    })
}

/// Make `out` the conversion of `source`, unless its stamp says it already is. A
/// missing source with a cache present keeps the cache (a game shipped without
/// its sources). A failing converter leaves the old cache as it was; a swap that
/// fails partway leaves the cache unstamped, so the next call converts again.
/// What the conversion skipped is warned of as it runs, and once per process
/// from the cache (`record`), the source present or not.
pub fn refresh(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    // Asked once: the program's lookup and version serve both checks below. A
    // missing source is served from its cache, so its program is never asked: a
    // player shipped without its `.blend` sources never starts Blender.
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
    // Held until this returns: a conversion of the same cache in another
    // process (or thread) waits, then finds this one's result current.
    let _lock = lock::hold(out);
    // Stamped again under the lock: what waited may have been converted
    // meanwhile, or the source saved, and the stamp names what the converter reads.
    let Some(stamp) = stale(converter, source, out, tool)? else {
        return Ok(());
    };
    let tag = record::folder_tag(out, &stamp);
    match stage::convert(converter, source, out, (&stamp, &tag)) {
        Ok(converted) => {
            let stored = record::Stored::written(&stamp, &tag, &converted);
            record::report(source, out, &stored);
        }
        // Without a lock, another process converted the same source meanwhile.
        Err(e) => {
            let stored = record::current(out, &stamp).ok_or(e)?;
            record::replay(source, out, &stored);
        }
    }
    Ok(())
}

/// Where the project at `root` caches the source at `rel` (project-relative): the
/// whole of `rel` under [`IMPORT_CACHE_DIR`], plus `.glb`. Nothing is stripped or
/// re-encoded, so distinct sources never share a cache (#1167).
pub fn cache_path(root: &Path, rel: &Path) -> PathBuf {
    with_suffix(&root.join(IMPORT_CACHE_DIR).join(rel), ".glb")
}

/// True when `path` is inside an import cache: what the manifest and the content
/// browser leave out, since the source is the asset.
pub fn is_import_cache(path: &Path) -> bool {
    paths::to_slash(path).contains(IMPORT_CACHE_DIR)
}

/// `path` relative to `root`, when `root` is a project and `path` lies in it.
/// Rebuilt from its components, never by rewriting `\`: on Linux and macOS a `\`
/// is part of a file name, so `a\b.fbx` is not `a/b.fbx`. A name that is not
/// UTF-8 is refused, so every name the cache derives from it (`stage`'s siblings)
/// is exact.
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

/// `path` relative to the working directory, if it lies in it.
fn relative_to_cwd(path: &Path) -> Option<PathBuf> {
    let rel = if path.is_relative() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir().ok()?;
        match path.strip_prefix(&cwd) {
            Ok(rel) => rel.to_path_buf(),
            // Symlinks, `..`, Windows short names: compare the canonical forms.
            Err(_) => {
                let (path, cwd) = (
                    paths::canonicalize(path).ok()?,
                    paths::canonicalize(&cwd).ok()?,
                );
                path.strip_prefix(cwd).ok()?.to_path_buf()
            }
        }
    };
    Some(rel)
}

/// Outside a project: a file in the temp folder named by the source's absolute
/// path, so two sources with one file name never share it.
fn temp_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let digest = hash::fnv1a64(absolute.to_string_lossy().as_bytes());
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    std::env::temp_dir()
        .join("rusty-import")
        .join(format!("{digest:016x}-{name}.glb"))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Why the source at `path` could not be read, in the OS's words.
fn unreadable(path: &Path) -> ImportError {
    let why = std::fs::read(path).map_or_else(|e| e.to_string(), |_| "not a file".into());
    ImportError::Parse(format!("{}: {why}", path.display()))
}

fn write_error(path: &Path, e: &std::io::Error) -> ImportError {
    ImportError::Parse(format!("writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
