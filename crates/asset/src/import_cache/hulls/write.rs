use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusty_core::hulls::{self, Key};
use rusty_core::project;

#[derive(Default)]
struct Models {
    project: u64,
    by_sidecar: BTreeMap<PathBuf, BTreeSet<Key>>,
}

static MODELS: Mutex<Option<Models>> = Mutex::new(None);

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

pub(super) fn note(sidecar: PathBuf, keys: BTreeSet<Key>) {
    with(|m| m.by_sidecar.insert(sidecar, keys));
}

/// Returns the sidecars written. Fresh entries are taken under the models' lock,
/// so a returning call finds every entry another call took already written.
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
    // lint: allow(asset-write) the hulls it adds are already in `core::hulls`; the sidecar is read only when a model is parsed
    rusty_core::fs::write_atomic(path, bytes)
}
