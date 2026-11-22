use std::path::{Path, PathBuf};

use rusty_asset::convert::{
    cache_path, cache_path_as, converter_for, lock_cache, with_cache_files, IMPORT_CACHE_DIR,
};
use rusty_asset::texture_import::cache_path_in;
use rusty_core::paths;

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

pub(super) fn tree(
    root: &Path,
    rel: &Path,
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    if rel == Path::new(IMPORT_CACHE_DIR) {
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
            // The import walk (`asset::manifest::walk`) skips a symlinked folder too, so a
            // source in one would ship unconverted.
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
    converter_for(rel).is_some()
        || blender_leftover
        || crate::bake::report::in_report_dir(rel)
        || paths::has_extension(rel, "mp3")
}

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

pub(super) fn texture_caches(
    root: &Path,
    images: &[PathBuf],
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    for image in images {
        let cache = cache_path_in(root, image);
        // Checked before the lock: an image never compressed has no cache folder, and locking
        // a path in a missing folder fails with a false warning.
        if !cache.is_file() {
            continue;
        }
        let _lock = lock_cache(&cache);
        let rel = cache.strip_prefix(root).unwrap_or(&cache);
        file(root, rel, out, written)?;
    }
    Ok(())
}

pub(super) fn model_caches(
    root: &Path,
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    let mut models = Vec::new();
    rusty_asset::manifest::walk(&root.join(rusty_core::project::ASSETS_DIR), &mut |p| {
        if rusty_asset::is_model_path(p) {
            models.push(p.strip_prefix(root).unwrap_or(p).to_path_buf());
        }
    });
    models.sort();
    let suffixes = [
        rusty_asset::import_cache::hulls::SUFFIX,
        rusty_asset::cook::SUFFIX,
    ];
    let caches = models
        .iter()
        .flat_map(|m| suffixes.map(|s| cache_path_as(root, m, s)));
    for cache in caches {
        if cache.is_file() {
            file(
                root,
                cache.strip_prefix(root).unwrap_or(&cache),
                out,
                written,
            )?;
        }
    }
    Ok(())
}
