//! src/asset/import_cache/hulls/write.rs — an authoring build writes the
//! decompositions it ran into the hull sidecars of the models that have those
//! meshes (#1940). The player compiles none of this.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::core::hulls::{self, Key};
use crate::core::project;

/// The models this project imported: each one's sidecar and the keys of the
/// meshes it has now.
#[derive(Default)]
struct Models {
    /// `project::moves()` when the models were noted.
    project: u64,
    by_sidecar: BTreeMap<PathBuf, BTreeSet<Key>>,
}

static MODELS: Mutex<Option<Models>> = Mutex::new(None);

/// The noted models, forgotten first if the process opened another project.
fn with<T>(f: impl FnOnce(&mut Models) -> T) -> T {
    let mut guard = MODELS.lock().unwrap_or_else(|e| e.into_inner());
    let models = guard.get_or_insert_with(Models::default);
    let moves = project::moves();
    if models.project != moves {
        *models = Models {
            project: moves,
            ..Models::default()
        };
    }
    f(models)
}

/// Note that the model whose sidecar is `sidecar` has the meshes `keys`.
pub(super) fn note(sidecar: PathBuf, keys: BTreeSet<Key>) {
    with(|m| m.by_sidecar.insert(sidecar, keys));
}

/// Write each decomposition made since the last call into the sidecar of every
/// model with that mesh (see the module doc). Returns the sidecars written.
///
/// The fresh entries are taken under the models' lock, so a call that returns
/// finds every entry another call took already written.
pub fn save_pending() -> Vec<PathBuf> {
    with(|models| {
        let fresh = hulls::take_fresh();
        let mut written = Vec::new();
        if fresh.is_empty() {
            return written;
        }
        for (sidecar, keys) in &models.by_sidecar {
            let new: Vec<_> = fresh.iter().filter(|(k, _)| keys.contains(k)).collect();
            if new.is_empty() {
                continue;
            }
            let mut entries = std::fs::read(sidecar)
                .map(|bytes| super::parse(sidecar, &bytes))
                .unwrap_or_default();
            entries.retain(|k, _| keys.contains(k));
            entries.extend(new.into_iter().cloned());
            match write(sidecar, &super::file::encode(&entries)) {
                Ok(()) => written.push(sidecar.clone()),
                Err(e) => log::warn!("[Import] cannot write {}: {e}", sidecar.display()),
            }
        }
        written
    })
}

fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::core::fs::write_atomic(path, bytes)
}
