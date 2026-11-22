//! Like Unity's `AssetDatabase.Refresh`: brings every converted model, its cooked
//! geometry and every image's compressed copy up to date before returning.
//! Play's own refresh compresses no texture, so a project the editor never
//! opened draws uncompressed until this has run.

use std::path::{Path, PathBuf};

use crate::convert::{cached_gltf, conversion_allowed, convertible_sources};
use crate::texture_import::{self, Refreshed};

type Failed = Vec<(PathBuf, String)>;

#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    pub sources: Vec<PathBuf>,
    pub failed: Failed,
    pub cooked: usize,
    pub textures: Refreshed,
}

/// Models first, then images. A first run on a large project encodes for minutes
/// (BC7 about 8-11 core-seconds per 2048² map). Refused while a world plays on
/// this thread. A failing source is listed and the rest still run.
pub fn import_all(root: &Path) -> Result<Imported, String> {
    let (sources, mut failed) = models(root)?;
    let cooked = cook_models(root, &mut failed);
    let textures = texture_import::refresh_all(root);
    Ok(Imported {
        sources,
        failed,
        cooked,
        textures,
    })
}

/// [`import_all`], but stops before any image is encoded when a model fails, so
/// a broken model fails in seconds, not after minutes of encoding.
pub fn import_strict(root: &Path) -> Result<Imported, String> {
    let (sources, mut failed) = models(root)?;
    refuse(&failed)?;
    let cooked = cook_models(root, &mut failed);
    refuse(&failed)?;
    Ok(Imported {
        sources,
        failed,
        cooked,
        textures: texture_import::refresh_all(root),
    })
}

fn refuse(failed: &Failed) -> Result<(), String> {
    if failed.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = failed
        .iter()
        .map(|(source, e)| format!("{}: {e}", source.display()))
        .collect();
    Err(named.join("; "))
}

pub fn notice(root: &Path) -> String {
    format!(
        "import: writing {}'s import cache into the project; a first run \
         compresses every image (minutes for a large project)",
        root.display()
    )
}

/// Converts every stale model source under `root`; returns the failures.
fn models(root: &Path) -> Result<(Vec<PathBuf>, Failed), String> {
    if !conversion_allowed() {
        return Err("Play is running, and the import waits for Stop: stop first".into());
    }
    let sources = convertible_sources(root);
    let mut failed = Vec::new();
    for source in &sources {
        if let Err(e) = cached_gltf(source) {
            log::warn!("[Asset] {}: {e}", source.display());
            failed.push((source.clone(), e.to_string()));
        }
    }
    Ok((sources, failed))
}

/// Returns how many cooked files were written. Currency is the load's own test,
/// not the stamp alone, so an export never ships a file every launch would
/// refuse. Sources already in `failed` are skipped.
fn cook_models(root: &Path, failed: &mut Failed) -> usize {
    if !crate::cook::writes() {
        return 0;
    }
    let before = crate::cook::written();
    let mut models = Vec::new();
    crate::manifest::walk(root, &mut |path| {
        if crate::is_model_path(path) && !failed.iter().any(|(f, _)| f == path) {
            models.push(path.to_path_buf());
        }
    });
    models.sort();
    let placed = models
        .iter()
        .filter(|m| crate::cook::cooked_path(m).is_some());
    for model in placed {
        if let Err(e) = crate::import_own_file(model) {
            log::warn!("[Asset] {}: {e}", model.display());
            failed.push((model.clone(), e.to_string()));
        }
    }
    crate::cook::written() - before
}

#[cfg(test)]
#[path = "import_cook_tests.rs"]
mod cook_tests;
#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
