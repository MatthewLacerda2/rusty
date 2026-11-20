//! The model imports one rehydrate pass has looked up, by path.
//!
//! [`rusty_asset::imported`] answers from its cache, but each call still brings the
//! file's conversion up to date and reads its import settings before the hit; a
//! level instancing one model hundreds of times paid that once per instance. A
//! pass (a load, a Play snapshot's restore, the prefab propagation that follows
//! either) asks once per distinct file instead: nothing it does writes a model, so
//! the answer cannot change within it.
//!
//! The same lookup tells a restore whether a live mesh is still what its source
//! builds ([`Imports::current`]): the import cache's own per-file check (the
//! source's and its conversion's stamps, its settings, the files it borrows clips
//! from) runs once per file, and a re-parse hands out a new revision.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusty_asset::{ImportError, Imported};
use rusty_components::{MeshComponent, SharedMesh};

use crate::create::{primitive_revision, Primitive};

/// Each file's import (or why it failed), looked up on first use, and the files a
/// live mesh was found current against.
#[derive(Clone, Default)]
pub struct Imports {
    by_path: BTreeMap<String, Result<Imported, ImportError>>,
    confirmed: BTreeSet<String>,
}

impl Imports {
    /// The import of the model file at `path`, looked up once per pass.
    pub fn of(&mut self, path: &str) -> Result<Imported, ImportError> {
        self.by_path
            .entry(path.to_string())
            .or_insert_with(|| rusty_asset::imported(Path::new(path)))
            .clone()
    }

    /// Whether `mesh`'s geometry is what its source builds now: an asset's revision
    /// is its file's import's as the cache holds it (a file changed on disk, a
    /// failed import, a changed setting or a dropped entry re-parses under a new
    /// one), a primitive's the one of its (primitive, cut). A mesh of another kind
    /// (a scene-library mesh, which a restore syncs from the library) is current
    /// by its revision alone, which the caller compares.
    pub fn current(&mut self, mesh: &MeshComponent) -> bool {
        if mesh.primitive_type == "Asset" {
            let Some(file) = mesh.asset_ref.as_deref().and_then(super::asset_ref_file) else {
                return false;
            };
            let current = self.of(&file).is_ok_and(|i| i.revision == mesh.revision);
            if current {
                self.confirmed.insert(file);
            }
            return current;
        }
        match Primitive::parse(&mesh.primitive_type) {
            Some(p) => primitive_revision(p, mesh.subdivisions) == mesh.revision,
            None => true,
        }
    }

    /// Whether a collider's own `mesh` (which carries no revision) is current: a
    /// primitive always is; an asset's when a Mesh of the same file was found
    /// [`Self::current`] earlier in the pass, so its import is the one the World
    /// was built from. Otherwise it is rebuilt, which is never wrong.
    pub fn confirmed(&self, mesh: &SharedMesh) -> bool {
        match mesh.primitive_type.as_str() {
            "Asset" => mesh
                .asset_ref
                .as_deref()
                .and_then(super::asset_ref_file)
                .is_some_and(|file| self.confirmed.contains(&file)),
            _ => true,
        }
    }
}
