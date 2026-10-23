//! src/scene/model_writes.rs — the entity side of `core::fs::written` (#1242).
//!
//! When the engine re-writes a model in place (an FBX's conversion swapped in, an
//! `.obj` or glTF its writer noted), every entity whose `"Asset"` mesh names that
//! file re-reads it: the CPU mesh, its mesh collider's box and, through the dirty
//! flag, its GPU upload move together, as Unity updates every instance of a
//! re-imported model. The renderer's particle-mesh cache follows the same log for
//! emitters (`render::passes::particles::mesh_cache`).
//!
//! It runs on the edit-mode tick (`app::World::edit_frame`). Nothing writes a model
//! during Play, and a note made then waits for the first edit-mode tick after Stop,
//! so it lands on the restored edit scene rather than the discarded play one.

use crate::core::fs::written::{written_since, Stale};
use crate::scene::rehydrate::reimport_written;
use crate::scene::Scene;

/// How far into the write log the scene's meshes have been brought up to date.
pub struct ModelWrites {
    seen: u64,
}

impl Default for ModelWrites {
    /// Starts at the current generation: a scene is read fresh when it loads, so
    /// earlier notes are already in it.
    fn default() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
        }
    }
}

impl ModelWrites {
    /// Drain the notes made since the last call, and re-import every mesh in `scene`
    /// whose model file one names. Returns how many were rebuilt. A re-import that
    /// itself notes a write (its conversion re-ran) is re-read once more next call,
    /// which then finds the cache current and notes nothing.
    pub fn follow(&mut self, scene: &mut Scene) -> usize {
        let (now, paths) = written_since(self.seen);
        self.seen = now;
        reimport_written(scene, &Stale::new(paths))
    }
}

#[cfg(test)]
#[path = "model_writes_tests.rs"]
mod tests;
