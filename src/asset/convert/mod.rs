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
//! every file a new mtime, and a sim module reads no clock. Files a conversion
//! writes for the glTF to reference (FBX's embedded textures, #1082) live in a
//! `<cache>.glb.<digest>.textures/` folder beside it and are re-made with it.
//!
//! **How it is re-made.** A converter writes plain files into a fresh staging
//! folder, and `stage` swaps the set into place, so a reader in another process
//! finds the set whole when it opens the `.glb` (#1129). A lock on the source
//! keeps two conversions of it from interleaving. Atomicity is this module's,
//! never a converter's.
//!
//! **Adding a converter** is one row in [`CONVERTERS`]: an extension and a
//! function that writes a glTF where its [`Staging`] says.

mod fbx;
pub mod glb;
mod stage;

pub use stage::{textures_dirs, Staging};

use super::ImportError;
use crate::core::{hash, paths, project};
use std::path::{Path, PathBuf};

/// Where an open project keeps converted sources, relative to its root: a folder
/// of [`project::GENERATED_DIR`].
pub const IMPORT_CACHE_DIR: &str = "assets/generated/imported";

/// One source format the importer converts to glTF.
pub struct Converter {
    /// The source extension, lowercase.
    pub extension: &'static str,
    /// What does the conversion, recorded in the stamp.
    pub name: &'static str,
    /// Bump when the converter's output changes, so every cached file re-makes.
    pub version: u32,
    /// Read `source` and write plain files where `to` says: the glTF (binary or
    /// not) at [`Staging::glb`], and the files it references that the conversion
    /// makes (FBX's embedded textures) in [`Staging::textures`]. [`refresh`] swaps
    /// them into place whole, and removes what an earlier conversion left.
    pub convert: fn(source: &Path, to: &Staging) -> Result<(), ImportError>,
}

/// Every converted format.
pub const CONVERTERS: &[Converter] = &[Converter {
    extension: "fbx",
    name: "ufbx",
    // 2: animation stacks become glTF animations (#1081).
    // 3: a rest pose splits like its keys, keeping a negative scale's signs (#1110).
    // 4: embedded textures are extracted beside the `.glb` (#1082).
    // 5: the textures folder is named by the conversion's digest (#1129).
    version: 5,
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
/// its sources). A failed conversion leaves the old cache as it was.
pub fn refresh(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    let parse = |e: std::io::Error| ImportError::Parse(format!("{}: {e}", source.display()));
    let file = match std::fs::File::open(source) {
        Ok(file) => file,
        Err(_) if out.is_file() => return Ok(()),
        Err(e) => return Err(parse(e)),
    };
    // A conversion of this source in another process (or thread) finishes first:
    // an advisory lock, released when `file` drops. Where the filesystem has none,
    // `stage` still keeps the set whole in most interleavings.
    let _ = file.lock();
    let bytes = std::fs::read(source).map_err(parse)?;
    let stamp = stamp(converter, &bytes);
    if up_to_date(out, &stamp) {
        return Ok(());
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| write_error(dir, &e))?;
    }
    match stage::convert(converter, source, out, &stamp) {
        // Another process converted the same source meanwhile.
        Err(_) if up_to_date(out, &stamp) => Ok(()),
        result => result,
    }
}

/// True when `out` exists and its stamp is `stamp`.
fn up_to_date(out: &Path, stamp: &str) -> bool {
    out.is_file() && std::fs::read_to_string(stamp_path(out)).is_ok_and(|s| s == stamp)
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
    with_suffix(out, ".source")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
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
