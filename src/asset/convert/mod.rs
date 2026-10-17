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
//! `project.rusty`), at [`IMPORT_CACHE_DIR`]`/<source path under assets/>.glb`,
//! beside the project's other generated assets: whether that folder is versioned
//! or ignored (#866) the cache follows, and a shipped game whose sources were
//! stripped still finds it. Anywhere else (a test, a file outside the project) the
//! glTF is written to the system temp folder, so a conversion never writes into a
//! tree that is not a project.
//!
//! **When it is re-made.** A `<cache>.glb.source` stamp beside each cached file
//! records the converter, its [`Converter::version`] and a digest of the source's
//! bytes; any difference re-converts. Content, not timestamps: a fresh clone gives
//! every file a new mtime, and a sim module reads no clock.
//!
//! **Adding a converter** is one row in [`CONVERTERS`]: an extension and a
//! function that writes a glTF to the path it is given.

mod fbx;
pub mod glb;

use super::ImportError;
use crate::core::{hash, paths, project};
use std::path::{Path, PathBuf};

/// Where an open project keeps converted sources, relative to its root.
pub const IMPORT_CACHE_DIR: &str = "assets/generated/imported";

/// One source format the importer converts to glTF.
pub struct Converter {
    /// The source extension, lowercase.
    pub extension: &'static str,
    /// What does the conversion, recorded in the stamp.
    pub name: &'static str,
    /// Bump when the converter's output changes, so every cached file re-makes.
    pub version: u32,
    /// Read `source`, write a glTF (binary or not) at `out`.
    pub convert: fn(source: &Path, out: &Path) -> Result<(), ImportError>,
}

/// Every converted format.
pub const CONVERTERS: &[Converter] = &[Converter {
    extension: "fbx",
    name: "ufbx",
    version: 1,
    convert: fbx::convert,
}];

/// The converter for `path`'s extension, if it is a converted format.
pub fn converter_for(path: &Path) -> Option<&'static Converter> {
    let ext = paths::extension(path);
    CONVERTERS.iter().find(|c| c.extension == ext)
}

/// The up-to-date glTF standing in for the converted source at `path`, converting
/// it first when the cache is missing or stale. Cached relative to the working
/// directory when that is a project (see the module doc).
pub fn cached_gltf(path: &Path) -> Result<PathBuf, ImportError> {
    let converter = converter_for(path)
        .ok_or_else(|| ImportError::UnsupportedFormat(paths::extension(path)))?;
    let root = Path::new(".");
    let out = match project_relative(root, path) {
        Some(rel) => cache_path(root, &rel),
        None => temp_path(path),
    };
    refresh(converter, path, &out)?;
    Ok(out)
}

/// Make `out` the conversion of `source`, unless its stamp says it already is. A
/// missing source with a cache present keeps the cache (a game shipped without
/// its sources).
pub fn refresh(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    let bytes = match std::fs::read(source) {
        Ok(bytes) => bytes,
        Err(_) if out.is_file() => return Ok(()),
        Err(e) => return Err(ImportError::Parse(format!("{}: {e}", source.display()))),
    };
    let stamp = stamp(converter, &bytes);
    let stamp_path = stamp_path(out);
    if out.is_file() && std::fs::read_to_string(&stamp_path).is_ok_and(|s| s == stamp) {
        return Ok(());
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| write_error(dir, &e))?;
    }
    // Written beside `out` and renamed into place, so a reader in another process
    // never sees half a file.
    // Still ending in `.glb`: an exporter may insist on its extension.
    let partial = out.with_extension(format!("{}.partial.glb", std::process::id()));
    (converter.convert)(source, &partial)?;
    std::fs::rename(&partial, out).map_err(|e| write_error(out, &e))?;
    std::fs::write(&stamp_path, stamp).map_err(|e| write_error(&stamp_path, &e))
}

/// Where the project at `root` caches the source at `rel` (project-relative):
/// under [`IMPORT_CACHE_DIR`], minus the leading `assets/`, plus `.glb`.
pub fn cache_path(root: &Path, rel: &str) -> PathBuf {
    let under = rel.strip_prefix("assets/").unwrap_or(rel);
    root.join(IMPORT_CACHE_DIR).join(format!("{under}.glb"))
}

/// True when `path` is inside an import cache: what the manifest and the content
/// browser leave out, since the source is the asset.
pub fn is_import_cache(path: &Path) -> bool {
    paths::to_slash(path).contains(IMPORT_CACHE_DIR)
}

/// `path` relative to `root`, `/`-separated, when `root` is a project and `path`
/// lies in it.
fn project_relative(root: &Path, path: &Path) -> Option<String> {
    if !root.join(project::PROJECT_FILE).is_file() {
        return None;
    }
    let rel = if path.is_relative() {
        paths::to_slash(path)
    } else {
        let cwd = std::env::current_dir().ok()?;
        paths::relativize(&path.to_string_lossy(), &cwd)
    };
    let rel = rel.strip_prefix("./").unwrap_or(&rel).to_string();
    (Path::new(&rel).is_relative() && !rel.split('/').any(|c| c == "..")).then_some(rel)
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

fn stamp_path(out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".source");
    PathBuf::from(name)
}

/// What the stamp records: the converter, its version and the source's digest.
fn stamp(converter: &Converter, source: &[u8]) -> String {
    let digest = hash::fnv1a64(source);
    format!("{} {} {digest:016x}\n", converter.name, converter.version)
}

fn write_error(path: &Path, e: &std::io::Error) -> ImportError {
    ImportError::Parse(format!("writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
