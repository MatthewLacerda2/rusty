//! Re-binding a live skeleton to a re-imported rig (#1252).
//!
//! When a skinned model is re-written, every entity built from it takes the new
//! mesh and rig, and its bones are rebuilt from the new rig at its rest pose. What
//! a designer authored on the old bones survives the way a save and load keeps it
//! (`persist`): per-bone overrides and bone bodies by bone name, and every entity
//! hung from a bone hung back by the same rule as a load (`hang`). A model
//! re-written as static keeps no bones, so what was authored on them is dropped,
//! with a warning.

use std::collections::BTreeSet;

use super::hang::Hung;
use super::persist::{strip, Why};
use crate::scene::Scene;

impl Scene {
    /// Take `owner`'s skeleton down: its bone overrides and bodies go to the mesh's
    /// `BoneBinding` (which [`Scene::sync_skeleton`] applies), the entities hung
    /// from its bones are parked under `owner` and returned with their bone
    /// chains, and the bones are despawned. Empty for an entity with no skin.
    pub(crate) fn take_skeleton(&mut self, owner: u32) -> Vec<Hung> {
        let Some(bones) = self
            .world
            .mesh(owner)
            .filter(|m| m.skin.is_some())
            .map(|m| m.skeleton.bones.clone())
        else {
            return Vec::new();
        };
        // The documents `strip` needs: the owner, its bones, and what hangs from a
        // bone (descendants of those keep their parent, so are not needed).
        let bone_set: BTreeSet<u32> = bones.iter().copied().collect();
        let mut ids = vec![owner];
        ids.extend(bones.iter().copied());
        for &bone in &bones {
            let hung = self.world.children(bone);
            ids.extend(hung.into_iter().filter(|c| !bone_set.contains(c)));
        }
        let mut docs: Vec<_> = ids
            .into_iter()
            .filter_map(|id| self.world.entity_document(id))
            .collect();
        strip(&mut docs, Why::Reimported);

        let mut hung = Vec::new();
        for doc in &mut docs {
            if doc.id == owner {
                if let (Some(saved), Some(mut mesh)) = (&doc.mesh, self.world.mesh_mut(owner)) {
                    mesh.skeleton.overrides = saved.skeleton.overrides.clone();
                    mesh.skeleton.bodies = saved.skeleton.bodies.clone();
                }
            } else if !doc.parent_bone.is_empty() {
                hung.push((doc.id, std::mem::take(&mut doc.parent_bone)));
            }
        }
        for (child, _) in &hung {
            let _ = self.set_parent(*child, Some(owner));
        }
        self.despawn_skeleton(owner);
        if let Some(mut mesh) = self.world.mesh_mut(owner) {
            mesh.skeleton.bones.clear();
        }
        hung
    }

    /// Build `owner`'s skeleton from its (new) rig and hang `hung` back on it
    /// ([`Scene::attach_to_bones`]). Returns the bones the rig no longer has that
    /// something hung from, each warned about. A mesh left unskinned drops the bone
    /// overrides and bodies it carried, warning about the bodies.
    pub(crate) fn restore_skeleton(&mut self, owner: u32, hung: Vec<Hung>) -> Vec<String> {
        let parked: Vec<u32> = hung.iter().map(|(id, _)| *id).collect();
        self.sync_skeleton_past(owner, &parked);
        let missing = self.attach_to_bones(hung);
        if let Some(mut mesh) = self.world.mesh_mut(owner).filter(|m| m.skin.is_none()) {
            mesh.skeleton.overrides.clear();
            let bodies: Vec<String> = std::mem::take(&mut mesh.skeleton.bodies)
                .into_keys()
                .collect();
            if !bodies.is_empty() {
                log::warn!(
                    "[Asset] entity {owner}'s re-imported model is no longer skinned; the \
                     Rigidbody / Joint on its bones {bodies:?} (its ragdoll) were dropped."
                );
            }
        }
        missing
    }
}
