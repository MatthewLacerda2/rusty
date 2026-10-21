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
//! records the converter, its [`Converter::version`] and a digest of the source's
//! bytes; any difference re-converts. Content, not timestamps: a fresh clone gives
//! every file a new mtime, and a sim module reads no clock. Files a conversion
//! writes for the glTF to reference (FBX's embedded textures, #1082) live in a
//! `<cache>.glb.<digest>.textures/` folder beside it and are re-made with it.
//!
//! **How it is re-made.** A converter writes plain files into a fresh staging
//! folder, and `stage` swaps the set into place, so a reader in another process
//! finds the set whole when it opens the `.glb` (#1129). A `<cache>.lock` file
//! keeps two conversions of one cache from interleaving; a cache already current
//! is read without taking it (#1166). Atomicity is this module's, never a
//! converter's.
//!
//! **What a conversion left out.** A converter never logs: it returns what it
//! skipped on purpose (an unsupported texture format, a take that would not
//! bake) in [`Converted`], and fails on anything else, so a cache missing a part
//! by accident is never stamped current (#1168). [`refresh`] warns of each skip,
//! and stores them under the stamp so a cached load warns again, as Unity shows
//! a model's import warnings on every reimport.
//!
//! **Adding a converter** is one row in [`CONVERTERS`]: an extension and a
//! function that writes a glTF where its [`Staging`] says.

mod fbx;
pub mod glb;
mod lock;
mod stage;

pub use stage::{textures_dirs, Staging};

use super::ImportError;
use crate::core::{hash, paths, project};
use std::path::{Component, Path, PathBuf};

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
    /// them into place whole, and removes what an earlier conversion left. Any
    /// failure is an `Err`; only what is left out on purpose goes in [`Converted`].
    pub convert: fn(source: &Path, to: &Staging) -> Result<Converted, ImportError>,
}

/// What a conversion reports beside the files it wrote.
#[derive(Debug, Default, PartialEq)]
pub struct Converted {
    /// What it left out on purpose, each with its reason.
    pub skipped: Vec<Skip>,
}

/// One part of a source a conversion left out on purpose.
#[derive(Clone, Debug, PartialEq)]
pub struct Skip {
    /// What was left out (`FBX texture "Skin"`).
    pub what: String,
    /// Why (`embeds a .tga, not an image format the importer reads`).
    pub why: String,
}

impl Skip {
    /// The skip as one line: what is warned of, and stored under the stamp.
    fn line(&self) -> String {
        format!("skipped {}: {}", self.what, self.why).replace(['\n', '\r'], " ")
    }
}

/// Every converted format.
pub const CONVERTERS: &[Converter] = &[Converter {
    extension: "fbx",
    name: "ufbx",
    // 2: animation stacks become glTF animations (#1081).
    // 3: a rest pose splits like its keys, keeping a negative scale's signs (#1110).
    // 4: embedded textures are extracted beside the `.glb` (#1082).
    // 5: the textures folder is named by the conversion's digest (#1129).
    // 6: a texture that fails to extract fails the conversion (#1168).
    version: 6,
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
/// its sources). A failing converter leaves the old cache as it was; a swap that
/// fails partway leaves the cache unstamped, so the next call converts again.
/// What the conversion skipped is warned of, whether it ran now or earlier.
pub fn refresh(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    let bytes = match std::fs::read(source) {
        Ok(bytes) => bytes,
        Err(_) if out.is_file() => return Ok(()),
        Err(e) => return Err(ImportError::Parse(format!("{}: {e}", source.display()))),
    };
    let stamp = stamp(converter, &bytes);
    if let Some(skipped) = current(out, &stamp) {
        report(source, &skipped);
        return Ok(());
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| write_error(dir, &e))?;
    }
    // Held until this returns: a conversion of the same cache in another
    // process (or thread) waits, then finds this one's result current.
    let _lock = lock::hold(out);
    let skipped = match current(out, &stamp) {
        Some(skipped) => skipped,
        None => stage::convert(converter, source, out, &stamp)?
            .skipped
            .iter()
            .map(Skip::line)
            .collect(),
    };
    report(source, &skipped);
    Ok(())
}

/// Warn of each part of `source` its conversion left out.
fn report(source: &Path, skipped: &[String]) {
    for skip in skipped {
        log::warn!("[Asset] {}: {skip}", source.display());
    }
}

/// What the conversion cached at `out` skipped, when `out` exists and is stamped
/// `stamp`; `None` when it must be converted.
fn current(out: &Path, stamp: &str) -> Option<Vec<String>> {
    let record = std::fs::read_to_string(stamp_path(out)).ok()?;
    let (found, skipped) = split_record(&record);
    (out.is_file() && found == stamp).then(|| skipped.lines().map(str::to_string).collect())
}

/// What a stamp file holds: the stamp, then one line per skip.
fn record(stamp: &str, converted: &Converted) -> String {
    let mut record = stamp.to_string();
    for skip in &converted.skipped {
        record.push_str(&skip.line());
        record.push('\n');
    }
    record
}

/// A stamp file's stamp (its first line, newline included) and the skips after it.
fn split_record(record: &str) -> (&str, &str) {
    let end = record.find('\n').map_or(record.len(), |i| i + 1);
    record.split_at(end)
}

/// Where the project at `root` caches the source at `rel` (project-relative,
/// `/`-separated): the whole of `rel` under [`IMPORT_CACHE_DIR`], plus `.glb`.
/// Nothing is stripped, so distinct sources never share a cache (#1167).
pub fn cache_path(root: &Path, rel: &str) -> PathBuf {
    root.join(IMPORT_CACHE_DIR).join(format!("{rel}.glb"))
}

/// True when `path` is inside an import cache: what the manifest and the content
/// browser leave out, since the source is the asset.
pub fn is_import_cache(path: &Path) -> bool {
    paths::to_slash(path).contains(IMPORT_CACHE_DIR)
}

/// `path` relative to `root`, its components joined by `/`, when `root` is a
/// project and `path` lies in it. Joined by component, never by rewriting `\`:
/// on Linux and macOS a `\` is part of a file name, and `a\b.fbx` is not `a/b.fbx`.
fn project_relative(root: &Path, path: &Path) -> Option<String> {
    if !root.join(project::PROJECT_FILE).is_file() {
        return None;
    }
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
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
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
