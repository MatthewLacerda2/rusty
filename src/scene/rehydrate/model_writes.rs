//! src/scene/rehydrate/model_writes.rs — the entity side of `core::fs::written` (#1242).
//!
//! When the engine re-writes a model in place (an FBX's conversion swapped in, an
//! `.obj` or glTF its writer noted), every entity whose `"Asset"` mesh names that
//! file re-reads it: the CPU mesh, its mesh collider's box and, through the fresh
//! geometry revision it takes (`MeshRevision`, #1540), every renderer's GPU upload
//! move together, as Unity updates every instance of a re-imported model; a
//! whole-model instance is rebuilt from the new import, node layout and all
//! (#1637, `prefab::restamp`). The
//! renderer's particle-mesh cache follows the same log for emitters (`render::passes::particles::mesh_cache`).
//!
//! It runs on the edit frame (`app::GameWorld::edit_frame`), every frame outside
//! Play, stepped or frozen. A model written during Play (an `.obj` or glTF writer;
//! conversion itself waits for Stop, #1253) has its note wait for the first edit
//! frame after Stop, and by then the restored edit scene was read from disk after it
//! (`Scene::meshes_read_at`), so nothing is read twice.

use std::collections::{BTreeMap, BTreeSet};

use crate::core::fs::written::{note_written, written_since, Stale};
use crate::scene::prefab::restamp::restamp_models;
use crate::scene::rehydrate::{reimport_model_except, reimport_written, Rig};
use crate::scene::Scene;

/// How far into the write log the scene's meshes have been brought up to date.
pub struct ModelWrites {
    seen: u64,
}

impl Default for ModelWrites {
    /// Starts at the current generation: whatever built the scene so far read its
    /// models fresh. A later scene load moves the floor up through
    /// `Scene::meshes_read_at`.
    fn default() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
        }
    }
}

impl ModelWrites {
    /// Drain the model notes made since the last call or the scene's last load,
    /// whichever is later, and re-import every mesh in `scene` whose model file one
    /// names. Returns how many were rebuilt. A re-import that itself notes a write
    /// (its conversion re-ran) is re-read once more next call, which then finds the
    /// cache current and notes nothing.
    pub fn follow(&mut self, scene: &mut Scene) -> usize {
        let (now, mut paths) = written_since(self.seen.max(scene.meshes_read_at));
        self.seen = now;
        let read = std::mem::take(&mut scene.meshes_reimported);
        paths.retain(|path| crate::asset::is_model_path(path) && !already_read(&read, path));
        if paths.is_empty() {
            return 0;
        }
        // A whole-model instance of a re-written model is rebuilt from it (#1637).
        let stale = Stale::new(paths);
        let refit =
            |scene: &mut Scene, skip: &_| reimport_written(scene, &stale, skip) + skip.len();
        restamp_models(scene, |source| stale.matches(source), refit)
    }
}

/// Whether the scene re-imported `path` itself at its latest note (`read`, from
/// [`reimport_noted`]): nothing wrote it again since.
fn already_read(read: &BTreeMap<String, u64>, path: &str) -> bool {
    read.get(path)
        .is_some_and(|&at| !written_since(at).1.iter().any(|p| p == path))
}

/// Re-import the meshes in `scene` instanced from the model at `path` now, for a
/// setting that changes what the model imports as (the Scale Factor, #1398), and
/// note the model as written so every other cache keyed by it re-reads it too.
/// The scene records the note it made, so its own [`ModelWrites::follow`] skips it
/// and the meshes are not read twice, whatever other notes are waiting. Returns the
/// ids of the entities rebuilt.
pub fn reimport_noted(scene: &mut Scene, path: &str, rig: Rig) -> Vec<u32> {
    reimport_noted_except(scene, path, rig, &BTreeSet::new())
}

/// [`reimport_noted`] leaving out the entities in `skip` (a linked model
/// instance's parts, which its rebuild re-imports, #1637).
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
