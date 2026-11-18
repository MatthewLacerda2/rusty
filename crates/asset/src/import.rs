//! crates/asset/src/import.rs — the import, run on purpose (#2319): every converted model and every image's
//! compressed copy under a folder brought up to date in the import cache, as
//! Unity's `AssetDatabase.Refresh` imports everything before it returns.
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
    /// The model sources that failed to convert, with why; each was warned of.
    pub failed: Failed,
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
    let (sources, failed) = models(root)?;
    let textures = texture_import::refresh_all(root);
    Ok(Imported {
        sources,
        failed,
        textures,
    })
}

/// [`import_all`], stopping before any image is encoded when a model fails: an
/// export's and `make import`'s import (`dev::export::import`), which fail on a
/// broken model, so they fail in seconds, not after minutes of encoding. The
/// error names every model that failed.
pub fn import_strict(root: &Path) -> Result<Imported, String> {
    let (sources, failed) = models(root)?;
    if !failed.is_empty() {
        let named: Vec<String> = failed
            .iter()
            .map(|(source, e)| format!("{}: {e}", source.display()))
            .collect();
        return Err(named.join("; "));
    }
    Ok(Imported {
        sources,
        failed,
        textures: texture_import::refresh_all(root),
    })
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

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
