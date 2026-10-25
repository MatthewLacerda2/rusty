//! src/scene/rehydrate/model_writes.rs — the entity side of `core::fs::written` (#1242).
//!
//! When the engine re-writes a model in place (an FBX's conversion swapped in, an
//! `.obj` or glTF its writer noted), every entity whose `"Asset"` mesh names that
//! file re-reads it: the CPU mesh, its mesh collider's box and, through the dirty
//! flag, its GPU upload move together, as Unity updates every instance of a
//! re-imported model. The renderer's particle-mesh cache follows the same log for
//! emitters (`render::passes::particles::mesh_cache`).
//!
//! It runs on the edit frame (`app::GameWorld::edit_frame`), every frame outside
//! Play, stepped or frozen. A model written during Play (an `.obj` or glTF writer;
//! conversion itself waits for Stop, #1253) has its note wait for the first edit
//! frame after Stop, and by then the restored edit scene was read from disk after it
//! (`Scene::meshes_read_at`), so nothing is read twice.

use crate::core::fs::written::{note_written, written_since, Stale};
use crate::scene::rehydrate::{reimport_model_ids, reimport_written, Rig};
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
        paths.retain(|path| crate::asset::is_model_path(path));
        if paths.is_empty() {
            return 0;
        }
        reimport_written(scene, &Stale::new(paths))
    }
}

/// Re-import the meshes in `scene` instanced from the model at `path` now, for a
/// setting that changes what the model imports as (the Scale Factor, #1398), and
/// note the model as written so every other cache keyed by it re-reads it too.
/// The scene's own [`ModelWrites::follow`] then skips that note, so the meshes are
/// not read twice; when another model's note is still waiting it is left to the
/// follow, which re-reads both. Returns the ids of the entities rebuilt.
pub fn reimport_noted(scene: &mut Scene, path: &str, rig: Rig) -> Vec<u32> {
    let (_, pending) = written_since(scene.meshes_read_at);
    let others = pending
        .iter()
        .any(|p| p != path && crate::asset::is_model_path(p));
    note_written(path);
    let ids = reimport_model_ids(scene, path, rig);
    if !others {
        scene.meshes_read_at = written_since(u64::MAX).0;
    }
    ids
}

#[cfg(test)]
#[path = "model_writes_tests.rs"]
mod tests;
