//! The copy half of an export: a project's files into the output, in sorted order,
//! convertible sources left out, each import cache copied under its lock (a
//! model's, and an image's compressed copy, #1574).

use std::path::{Path, PathBuf};

use crate::asset::convert::{
    cache_path, cache_path_as, converter_for, lock_cache, with_cache_files, IMPORT_CACHE_DIR,
};
use crate::asset::texture_import::CACHE_SUFFIX;
use crate::core::paths;

/// Copy the file `rel` of the project at `root` to the same place under `out`,
/// and record it in `written`.
pub(super) fn file(
    root: &Path,
    rel: &Path,
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    let to = out.join(rel);
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let from = root.join(rel);
    std::fs::copy(&from, &to).map_err(|e| format!("copying {}: {e}", from.display()))?;
    written.push(paths::to_slash(rel));
    Ok(())
}

/// Copy every file under the folder `rel` of the project at `root` but the import
/// cache ([`caches`] copies it) and what [`left_behind`] names, visiting each
/// folder's entries by name. A symlinked folder fails the export, naming it: the
/// import walk (`asset::manifest::walk`) does not follow one either, so a source
/// in it would ship unconverted. A symlinked file is copied as the file it names.
pub(super) fn tree(
    root: &Path,
    rel: &Path,
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    if rel == paths::native(IMPORT_CACHE_DIR) {
        return Ok(());
    }
    let dir = root.join(rel);
    let read = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut names = read
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    names.sort();
    for name in names {
        let rel = rel.join(name);
        let path = root.join(&rel);
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .file_type();
        if kind.is_dir() {
            tree(root, &rel, out, written)?;
        } else if kind.is_symlink() && path.is_dir() {
            return Err(format!(
                "{}: a symlinked folder; copy its files into the project instead",
                rel.display()
            ));
        } else if !left_behind(&rel) && path.is_file() {
            file(root, &rel, out, written)?;
        }
    }
    Ok(())
}

/// Whether an export leaves the file `rel` behind: a source a converter reads,
/// a backup or temp file Blender writes beside a `.blend` (`.blend1`, `.blend2`,
/// …, and `.blend@`), which holds the same source, or an `.mp3`, audio's source
/// format: the export's refresh made its `.wav`, which ships, and one the refresh
/// skipped (its `.wav` already there, or it does not decode) ships neither way.
pub(super) fn left_behind(rel: &Path) -> bool {
    let blender_leftover = rel
        .extension()
        .and_then(|e| e.to_str())
        .and_then(|e| {
            e.to_ascii_lowercase()
                .strip_prefix("blend")
                .map(str::to_owned)
        })
        .is_some_and(|n| n == "@" || (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())));
    converter_for(rel).is_some() || blender_leftover || paths::has_extension(rel, "mp3")
}

/// Copy the import cache of each source in `sources` (project-relative), under
/// its lock, and return the cached glTFs copied. A cache whose source is gone is
/// not among them, so it is left behind.
pub(super) fn caches(
    root: &Path,
    sources: &[PathBuf],
    out: &Path,
    written: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut glbs = Vec::new();
    for source in sources {
        let cache = cache_path(root, source);
        with_cache_files(&cache, |files| {
            for from in files {
                let rel = from.strip_prefix(root).unwrap_or(from);
                file(root, rel, out, written)?;
            }
            Ok::<_, String>(())
        })?;
        if cache.is_file() {
            glbs.push(paths::to_slash(cache.strip_prefix(root).unwrap_or(&cache)));
        }
    }
    Ok(glbs)
}

/// Copy the compressed copy (`.bctex`, #1574) of each image in `images`
/// (project-relative) that has one, under its lock, so the exported game never
/// encodes. An image left uncompressed has none, and uploads as it is.
pub(super) fn texture_caches(
    root: &Path,
    images: &[PathBuf],
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    for image in images {
        let cache = cache_path_as(root, image, CACHE_SUFFIX);
        // Looked for before the lock, whose folder an image never compressed
        // lacks: a lock that cannot be taken warns once per process, falsely.
        if !cache.is_file() {
            continue;
        }
        let _lock = lock_cache(&cache);
        let rel = cache.strip_prefix(root).unwrap_or(&cache);
        file(root, rel, out, written)?;
    }
    Ok(())
}
