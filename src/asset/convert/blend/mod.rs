//! src/asset/convert/blend/ — `.blend` → glTF through Blender (#1072).
//!
//! Unity read FBX only and imported a `.blend` by having Blender export one behind
//! the scenes; rusty does the same with glTF, its one format inside. Blender 5 or
//! later runs headless (`run`) with the export settings pinned in one script
//! (`export.py`: +Y up, modifiers applied, animations and skins on), so two
//! machines with the same Blender make the same glTF from the same `.blend`. What
//! it writes is joined into the cached `.glb` (`pack`). The engine never parses a
//! `.blend` itself.
//!
//! Blender is a requirement (CLAUDE.md, *One format inside*): found as `locate`
//! says, and when it is missing or older than 5 the import fails saying what was
//! found and where to get it. Its version goes into the cache's stamp
//! ([`identify`]), so a different Blender re-converts; with no Blender at all, a
//! cache of the unchanged source still loads.
//!
//! A run of Blender has a time limit (`run`): ten seconds to say its version, ten
//! minutes to export unless `RUSTY_BLENDER_TIMEOUT` says otherwise. One past it is
//! stopped and fails, so a hung Blender never holds the editor or the cache's lock.
//!
//! **Textures.** Only the images a material uses are looked at. One packed into
//! the `.blend` (or generated in it) is written into the textures folder beside
//! the `.glb`, flat, under its identity: the image's name, unique within the
//! export (the first keeps it; a linked library's namesake takes `.1`), its
//! extension the format of its bytes. A scene names it
//! `model.blend#tex/<identity>` (#1130). An image that is a file on disk stays a
//! plain path to that file. Each linked library, and each image on disk, is a
//! dependency of the cache (#1189), and a changed one re-converts into a textures
//! folder of its own (`record::folder_tag`).
//!
//! **Authoring only.** A running game never starts Blender, not even to ask its
//! version: Play serves the cache as it stands (`gate::serve`). Only the refresh
//! before Play (`gate::refresh_sources`) converts, in the editor and, while a game
//! still ships with its sources (until #1267's export), in the standalone player.
//!
//! A converter never logs or drops a `Result`: what it leaves out on purpose is a
//! [`Skip`](super::Skip) in what it returns, and any failure fails the conversion
//! (#1172).
// A converter's (#1172): a `let _ =` or a discarded `.ok()` here would drop a
// failure the cache must not be stamped over, and a print would bypass the one
// warning path; the rest of the crate keeps `let _ =` for best-effort cleanup.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod locate;
mod pack;
mod run;

pub use locate::Search;

use super::stage::hook::{self, Step};
use super::{Converted, Staging};
use crate::asset::ImportError;
use std::path::Path;

/// The folder inside the staging folder Blender exports into.
const EXPORT_DIR: &str = "blender";

/// Convert the `.blend` at `source` into a `.glb` where `to` says, through the
/// Blender this process's environment finds.
pub fn convert(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    convert_with(&Search::from_env(), source, to)
}

/// The Blender this process converts with, by its version: what the stamp records.
pub fn identify() -> Result<String, ImportError> {
    Search::from_env().find().map(|blender| blender.version)
}

/// [`convert`] with the Blender `search` finds.
fn convert_with(search: &Search, source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let blender = search.find()?;
    let staging = to.glb.parent().unwrap_or_else(|| Path::new("."));
    // Absolute: Blender writes the image URIs relative to it, which `pack` joins.
    let dir = std::path::absolute(staging.join(EXPORT_DIR))
        .map_err(|e| ImportError::Parse(format!("{}: {e}", staging.display())))?;
    hook::io(Step::ReadSource)
        .and_then(|()| std::fs::create_dir(&dir))
        .map_err(|e| ImportError::Parse(format!("writing {}: {e}", dir.display())))?;
    run::export(&blender.path, source, &dir, search.export_timeout()?)?;
    pack::pack(&dir, to)
}

#[cfg(test)]
mod tests;
