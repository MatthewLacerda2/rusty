//! src/asset/import_cache/hulls/ — a model's hull sidecar: the convex
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
//! a stale entry is never used.
//!
//! **Written** by a dev build only ([`save_pending`], called at Play entry and
//! Stop): each decomposition the physics ran since the last call is added to the
//! sidecar of every imported model with that mesh, and the entries of meshes the
//! model no longer has are dropped. A shipped player reads sidecars and never
//! writes one. A mesh that came from no model (a primitive, a script-built mesh)
//! is decomposed once per session, as before.

pub mod file;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::super::mesh_data::ImportedAsset;
use crate::core::hulls::{self, Key};

/// What a hull sidecar's name adds to its model source's.
pub const SUFFIX: &str = ".hulls";

/// Each imported model's sidecar and the keys of the meshes it has now.
static MODELS: Mutex<BTreeMap<PathBuf, BTreeSet<Key>>> = Mutex::new(BTreeMap::new());

/// Where the hull sidecar of the model source `source` lives.
pub fn sidecar_of(source: &Path) -> Option<PathBuf> {
    crate::asset::convert::cache_location_as(source, SUFFIX).ok()
}

/// Note the meshes of the model `source` just parsed into `asset`, and keep the
/// pieces its sidecar holds for them. A sidecar that will not parse is warned of
/// and left for the next write to replace.
pub fn load(source: &Path, asset: &ImportedAsset) {
    let Some(sidecar) = sidecar_of(source) else {
        return;
    };
    let keys: BTreeSet<Key> = (asset.sub_meshes.iter())
        .map(|s| {
            let positions: Vec<[f32; 3]> = s.vertices.iter().map(|v| v.position).collect();
            hulls::key(&positions, &s.indices)
        })
        .collect();
    let entries = read(&sidecar);
    for (key, pieces) in entries.into_iter().filter(|(k, _)| keys.contains(k)) {
        hulls::keep(key, pieces);
    }
    let mut models = MODELS.lock().unwrap_or_else(|e| e.into_inner());
    models.insert(sidecar, keys);
}

/// Write each decomposition made since the last call into the sidecar of every
/// imported model with that mesh (dev builds; see the module doc). Returns the
/// sidecars written.
pub fn save_pending() -> Vec<PathBuf> {
    let fresh = hulls::take_fresh();
    if fresh.is_empty() {
        return Vec::new();
    }
    let models = MODELS.lock().unwrap_or_else(|e| e.into_inner());
    let mut written = Vec::new();
    for (sidecar, keys) in models.iter() {
        let new: Vec<_> = fresh.iter().filter(|(k, _)| keys.contains(k)).collect();
        if new.is_empty() {
            continue;
        }
        let mut entries = read(sidecar);
        entries.retain(|k, _| keys.contains(k));
        entries.extend(new.into_iter().cloned());
        match write(sidecar, &file::encode(&entries)) {
            Ok(()) => written.push(sidecar.clone()),
            Err(e) => log::warn!("[Import] cannot write {}: {e}", sidecar.display()),
        }
    }
    written
}

/// The entries of the sidecar at `path`: none when there is no file, and none,
/// with a warning, when it will not parse.
fn read(path: &Path) -> BTreeMap<Key, hulls::Pieces> {
    let Ok(bytes) = std::fs::read(path) else {
        return BTreeMap::new();
    };
    file::parse(&bytes).unwrap_or_else(|why| {
        log::warn!(
            "[Import] {}: {why}; its meshes decompose again",
            path.display()
        );
        BTreeMap::new()
    })
}

fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::core::fs::write_atomic(path, bytes)
}
