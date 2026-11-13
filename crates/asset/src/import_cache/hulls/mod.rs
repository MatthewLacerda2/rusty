//! crates/asset/src/import_cache/hulls/ — a model's hull sidecar: the convex
//! decompositions of its meshes, so a dynamic concave prop runs no V-HACD on the
//! first Play of a fresh launch (#1940).
//!
//! **Where.** Beside the model's cached glTF, at
//! `convert::cache_location_as(<source>, ".hulls")`: in an open project,
//! `assets/generated/imported/<source>.hulls` (shipped by an export, #1267);
//! elsewhere (a test, a file outside a project) the system temp folder.
//!
//! **Read** when the import cache parses a model ([`load`]): every entry whose mesh
//! the model still has goes into `core::hulls`, where the physics looks first.
//! Entries are keyed by each mesh's content, so a changed source simply misses, and
//! a stale entry is never used. A sidecar made by another decomposer
//! (`core::hulls::DECOMPOSER`) is refused like a malformed one.
//!
//! **Written** by an authoring build only (`core::hulls::WRITES`: the editor or
//! `dev`; `save_pending`, called at Play entry and Stop): each decomposition the
//! physics ran since the last call is added to the sidecar of every model this
//! project imported with that mesh, and the entries of meshes the model no longer
//! has are dropped. The models noted are forgotten when the process opens another
//! project (`core::project::moves`, #1269). A shipped player reads sidecars, never
//! writes one, and notes no models. A mesh that came from no model (a primitive, a
//! script-built mesh) is decomposed once per session, as before.

pub mod file;
#[cfg(any(feature = "editor", feature = "dev"))]
mod write;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::super::mesh_data::ImportedAsset;
use rusty_core::hulls::{self, Key};

#[cfg(any(feature = "editor", feature = "dev"))]
pub use write::save_pending;

/// What a hull sidecar's name adds to its model source's.
pub const SUFFIX: &str = ".hulls";

/// Where the hull sidecar of the model source `source` lives.
pub fn sidecar_of(source: &Path) -> Option<PathBuf> {
    crate::convert::cache_location_as(source, SUFFIX).ok()
}

/// Keep the pieces the sidecar of the model `source`, just parsed into `asset`,
/// holds for its meshes, and (authoring builds) note the model's meshes for
/// writing. A sidecar that will not parse is warned of and left for the next
/// write to replace. Without a sidecar the player does nothing more.
pub fn load(source: &Path, asset: &ImportedAsset) {
    let Some(sidecar) = sidecar_of(source) else {
        return;
    };
    let bytes = std::fs::read(&sidecar).ok();
    if bytes.is_none() && !hulls::WRITES {
        return;
    }
    let keys: BTreeSet<Key> = (asset.sub_meshes.iter())
        .map(|s| hulls::key(s.vertices.iter().map(|v| v.position), &s.indices))
        .collect();
    if let Some(bytes) = bytes {
        let entries = parse(&sidecar, &bytes);
        for (key, pieces) in entries.into_iter().filter(|(k, _)| keys.contains(k)) {
            hulls::keep(key, pieces);
        }
    }
    #[cfg(any(feature = "editor", feature = "dev"))]
    write::note(sidecar, keys);
}

/// The entries of the sidecar `bytes` read from `path`: none, with a warning,
/// when it will not parse.
fn parse(path: &Path, bytes: &[u8]) -> BTreeMap<Key, hulls::Pieces> {
    file::parse(bytes).unwrap_or_else(|why| {
        log::warn!(
            "[Import] {}: {why}; its meshes decompose again",
            path.display()
        );
        BTreeMap::new()
    })
}
