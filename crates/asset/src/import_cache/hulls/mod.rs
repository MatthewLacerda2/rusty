//! A model's convex decompositions, cached beside its glTF so a dynamic concave
//! prop runs no V-HACD on the first Play of a fresh launch. Entries are keyed by
//! mesh content, so a changed source misses. Only authoring builds write
//! (`save_pending`); a shipped player only reads.

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

pub const SUFFIX: &str = ".hulls";

/// Public for `physics`' sidecar tests too.
pub fn sidecar_of(source: &Path) -> Option<PathBuf> {
    crate::convert::cache_location_as(source, SUFFIX).ok()
}

/// A sidecar that will not parse is warned of and left for the next write to replace.
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

fn parse(path: &Path, bytes: &[u8]) -> BTreeMap<Key, hulls::Pieces> {
    file::parse(bytes).unwrap_or_else(|why| {
        log::warn!(
            "[Import] {}: {why}; its meshes decompose again",
            path.display()
        );
        BTreeMap::new()
    })
}
