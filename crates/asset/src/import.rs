//! crates/asset/src/import.rs — the import, run on purpose (#2319): every converted model and every image's
//! compressed copy under a folder brought up to date in the import cache, as
//! Unity's `AssetDatabase.Refresh` imports everything before it returns.
//!
//! The one routine behind `Assets.Refresh`, `make import` (`export
//! --import-only`), an export's first step (`dev::export`) and `make bench-scenes
//! IMPORT=1`. Play's own refresh (`convert::refresh_sources`)
//! does the models part; a standalone game (the player, a dev harness) compresses
//! no texture there, so a headless run on a project the editor never opened draws
//! every texture uncompressed until this has run on it.

use std::path::{Path, PathBuf};

use crate::convert::{cached_gltf, conversion_allowed, convertible_sources};
use crate::texture_import::{self, Refreshed};

/// What [`import_all`] did.
#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    /// Every model source looked at (each converted when its cache was stale), sorted.
    pub sources: Vec<PathBuf>,
    /// The model sources that failed to convert, with why; each was warned of.
    pub failed: Vec<(PathBuf, String)>,
    /// What the images' refresh did (`texture_import::refresh_all`).
    pub textures: Refreshed,
}

/// Bring every converted model and every image's compressed copy under `root` up
/// to date, writing the import cache into the project. Says so first, on the
/// console, since a first run on a large project encodes for minutes (BC7 at
/// about 8-11 core-seconds a 2048² map, `texture_import::encode`). Refused while
/// a world plays on this thread (`convert::conversion_allowed`): nothing is written
/// then, and the error says to stop first. A source that fails is warned of,
/// listed, and the rest still run.
pub fn import_all(root: &Path) -> Result<Imported, String> {
    if !conversion_allowed() {
        return Err("Play is running, and the import waits for Stop: stop first".into());
    }
    log::info!(
        "[Import] bringing {}'s import cache up to date: this writes into the project, \
         and a first run compresses every image (minutes for a large project)",
        root.display()
    );
    let sources = convertible_sources(root);
    let mut failed = Vec::new();
    for source in &sources {
        if let Err(e) = cached_gltf(source) {
            log::warn!("[Asset] {}: {e}", source.display());
            failed.push((source.clone(), e.to_string()));
        }
    }
    let textures = texture_import::refresh_all(root);
    Ok(Imported {
        sources,
        failed,
        textures,
    })
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
