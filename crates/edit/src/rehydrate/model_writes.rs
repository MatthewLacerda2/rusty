use std::collections::{BTreeMap, BTreeSet};

use crate::prefab::restamp::restamp_models;
use crate::rehydrate::{reimport_model_except, reimport_written, Rig};
use rusty_core::fs::written::{note_written, written_since, Stale};
use rusty_scene::Scene;

pub struct ModelWrites {
    seen: u64,
}

impl Default for ModelWrites {
    fn default() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
        }
    }
}

impl ModelWrites {
    pub fn follow(&mut self, scene: &mut Scene) -> usize {
        let (now, mut paths) = written_since(self.seen.max(scene.meshes_read_at));
        self.seen = now;
        let read = std::mem::take(&mut scene.meshes_reimported);
        paths.retain(|path| rusty_asset::is_model_path(path) && !already_read(&read, path));
        if paths.is_empty() {
            return 0;
        }
        let stale = Stale::new(paths);
        let refit =
            |scene: &mut Scene, skip: &_| reimport_written(scene, &stale, skip) + skip.len();
        restamp_models(scene, |source| stale.matches(source), refit)
    }
}

fn already_read(read: &BTreeMap<String, u64>, path: &str) -> bool {
    read.get(path)
        .is_some_and(|&at| !written_since(at).1.iter().any(|p| p == path))
}

pub fn reimport_noted_except(
    scene: &mut Scene,
    path: &str,
    rig: Rig,
    skip: &BTreeSet<u32>,
) -> Vec<u32> {
    let at = note_written(path);
    scene.meshes_reimported.insert(path.to_string(), at);
    reimport_model_except(scene, path, rig, skip)
}

#[cfg(test)]
#[path = "model_writes_tests.rs"]
mod tests;
