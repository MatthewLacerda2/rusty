//! src/scene/skeleton/ — bones are GameObjects (#453).
//!
//! A skinned mesh's skeleton exists in the scene as ordinary child entities of the
//! skinned entity, one per joint, named after the glTF joint node — Unity's model.
//! The Animator writes each bone's local Transform, scripts and physics may move
//! them after it, and the palette build reads their world matrices to skin the mesh.
//! Attaching something to a bone is plain parenting: the gun goes under `hand_r`.
//!
//! The skeleton is intrinsic to the asset, so it is **rebuilt from the model, never
//! saved**:
//! - [`Scene::sync_skeletons`] spawns (or re-binds by name) each skinned mesh's
//!   bones after a load, a prefab stamp or an asset instantiate.
//! - [`strip_bones`] takes them out of a saved document, keeping only what the
//!   designer authored: per-bone transform overrides (on the mesh's
//!   `BoneBinding`) and the entities parented under a bone, re-pointed at the
//!   skinned entity with their bone's name in `parent_bone`.
//! - [`take_bone_parents`] + [`Scene::attach_to_bones`] put those attachments back
//!   under their bones on load. A bone that no longer exists is warned about, never
//!   dropped silently.
//! - `palette` builds each skinned mesh's posed palette from the bones.

mod palette;
mod persist;
mod sync;

use std::collections::BTreeSet;

pub use persist::{strip_bones, take_bone_parents};

use crate::scene::Scene;

/// The `next_entity_id` a save records: the World's, minus the ids at its top
/// held by bones. Bones are respawned with fresh ids on every load, so without
/// this each load-and-save cycle would grow the saved counter by the bone count —
/// churn in a version-controlled scene file. Reusing a bone's id is safe: nothing
/// saved refers to a bone by id.
pub fn saved_next_id(scene: &Scene) -> u32 {
    let bones = scene.bone_ids();
    let mut next = scene.world.next_id();
    while next > 1 && bones.contains(&(next - 1)) {
        next -= 1;
    }
    next
}

impl Scene {
    /// Every bone entity of every skinned mesh in the scene.
    pub fn bone_ids(&self) -> BTreeSet<u32> {
        self.world
            .ids_with_mesh()
            .into_iter()
            .filter_map(|id| self.world.mesh(id).map(|m| m.skeleton.bones.clone()))
            .flatten()
            .collect()
    }

    /// The skinned entity whose skeleton `id` is a bone of, found by walking up
    /// the hierarchy; `None` for an entity that is not a bone.
    pub fn bone_owner(&self, id: u32) -> Option<u32> {
        let mut current = self.world.parent_id(id)?;
        loop {
            let owns = self
                .world
                .mesh(current)
                .is_some_and(|m| m.skeleton.bones.contains(&id));
            if owns {
                return Some(current);
            }
            current = self.world.parent_id(current)?;
        }
    }

    /// Despawn `owner`'s bones and everything under them — a skinned entity's
    /// skeleton goes with it.
    pub(crate) fn despawn_skeleton(&mut self, owner: u32) {
        let mut stack = match self.world.mesh(owner) {
            Some(mesh) => mesh.skeleton.bones.clone(),
            None => return,
        };
        while let Some(id) = stack.pop() {
            stack.extend(self.world.children(id));
            self.world.despawn(id);
        }
    }

    /// The bone of `owner`'s skeleton named `name` (the glTF joint name).
    pub fn find_bone(&self, owner: u32, name: &str) -> Option<u32> {
        let bones = self.world.mesh(owner)?.skeleton.bones.clone();
        bones
            .into_iter()
            .find(|&b| self.world.name(b).is_some_and(|n| *n == name))
    }
}

#[cfg(test)]
mod fixture;
#[cfg(test)]
mod persist_tests;
#[cfg(test)]
mod tests;
