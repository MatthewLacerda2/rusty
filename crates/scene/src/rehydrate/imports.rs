use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusty_asset::{ImportError, Imported};
use rusty_components::{MeshComponent, SharedMesh};

use crate::create::{primitive_revision, Primitive};

/// Each model file one pass looked up, once: nothing in a pass writes a model, so the
/// answer cannot change within it.
#[derive(Clone, Default)]
pub struct Imports {
    by_path: BTreeMap<String, Result<Imported, ImportError>>,
    confirmed: BTreeSet<String>,
}

impl Imports {
    pub fn of(&mut self, path: &str) -> Result<Imported, ImportError> {
        self.by_path
            .entry(path.to_string())
            .or_insert_with(|| rusty_asset::imported(Path::new(path)))
            .clone()
    }

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
            // A library mesh counts as current here: the restore's mesh sync checks its revision.
            None => true,
        }
    }

    /// A collider's own mesh has no revision: an asset's is current once a Mesh of its file
    /// was found [`Self::current`] earlier in the pass. Otherwise it is rebuilt, never wrong.
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
