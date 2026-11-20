//! crates/asset/src/import.rs — the import, run on purpose (#2319): every converted model and every image's
//! compressed copy under a folder brought up to date in the import cache, as
//! Unity's `AssetDatabase.Refresh` imports everything before it returns.
//!
//! Every model under the folder is imported too, so its cooked derived geometry
//! (`cook`) is current: a later load reads it instead of computing it.
//!
//! The one routine behind `Assets.Refresh` ([`import_all`]) and, stopping on a
//! broken model before any image ([`import_strict`]), `make import` (`export
//! --import-only`), an export's first step (`dev::export::import`) and `make
//! bench-scenes IMPORT=1`. Play's own refresh (`convert::refresh_sources`)
//! does the models part; a standalone game (the player, a dev harness) compresses
//! no texture there, so a `play` or `bench-scenes` run on a project the editor never opened draws
//! every texture uncompressed until this has run on it.

use std::path::{Path, PathBuf};

use crate::convert::{cached_gltf, conversion_allowed, convertible_sources};
use crate::texture_import::{self, Refreshed};

/// The sources that failed, each with why.
type Failed = Vec<(PathBuf, String)>;

/// What [`import_all`] did.
#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    /// Every model source looked at (each converted when its cache was stale), sorted.
    pub sources: Vec<PathBuf>,
    /// The model files that failed to convert or import, with why; each was
    /// warned of.
    pub failed: Failed,
    /// How many cooked files were written (`cook`): every model under the folder
    /// is imported as a load imports it, and a cooked file that read refuses
    /// (missing, stale, malformed) is written again.
    pub cooked: usize,
    /// What the images' refresh did (`texture_import::refresh_all`).
    pub textures: Refreshed,
}

/// Bring every converted model and every image's compressed copy under `root` up
/// to date, writing the import cache into the project, models first. A first run
/// on a large project encodes for minutes (BC7 at about 8-11 core-seconds a 2048²
/// map, `texture_import::encode`): a verb says so first ([`notice`]). Refused while
/// a world plays on this thread (`convert::conversion_allowed`): nothing is written
/// then, and the error says to stop first. A source that fails is warned of,
/// listed, and the rest still run.
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

/// [`import_all`], stopping before any image is encoded when a model fails: an
/// export's and `make import`'s import (`dev::export::import`), which fail on a
/// broken model, so they fail in seconds, not after minutes of encoding. The
/// error names every model that failed.
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

/// The error naming every model in `failed`, when there is one.
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

/// What an import of `root` says before it starts (a verb that runs it prints
/// it): that it writes into the project, and how long a first run takes.
pub fn notice(root: &Path) -> String {
    format!(
        "import: writing {}'s import cache into the project; a first run \
         compresses every image (minutes for a large project)",
        root.display()
    )
}

/// The first half of an import: every model source under `root` converted when
/// stale, and the ones that failed, each warned of. Refused during Play.
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

/// Bring the cooked file of every model file under `root` up to date, returning
/// how many were written. Each model is imported its own (`import_own`, no
/// borrowed clips retargeted) as a load imports it: a cooked file the read takes
/// (its stamp, every sub-mesh's parse digest, counts and positions) is left as it
/// is; one it refuses, stale or not, is cooked and written again. So currency is
/// the load's own test, not the stamp alone (`cook::is_stamped`), and an export
/// never ships a file every launch would refuse. A model that has a current file
/// is parsed for it, never cooked. Nothing is imported when this thread writes
/// no cooked file (`cook::writes`: a standalone game, a build without authoring)
/// or the model has no place for one (outside an open project). A source
/// already in `failed` (its conversion, warned of) is not tried again; a model
/// that will not import is warned of and added to it.
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
