//! Re-binding a live skeleton to a re-imported rig (#1252).
//!
//! When a skinned model is re-written, every entity built from it takes the new
//! mesh and rig, and its bones are rebuilt from the new rig at its rest pose. What
//! a designer authored on the old bones survives the way a save and load keeps it
//! (`persist`): per-bone overrides and bone bodies by bone name, and every entity
//! hung from a bone re-attached to the bone of the same name. One whose bone the
//! new rig lacks goes to the nearest surviving ancestor bone, or to the skinned
//! entity, with a warning naming the bone — Unity keeps an object parented to a
//! removed bone rather than dropping it.

use std::collections::{BTreeMap, BTreeSet};

use super::persist::strip_bones;
use crate::scene::Scene;

/// What [`Scene::take_skeleton`] lifted off a skeleton beyond what the mesh's
/// `BoneBinding` carries: the entities hung from its bones, and where each old
/// bone sat in the old rig, to find a fallback when the new rig drops one.
#[derive(Default)]
pub(crate) struct Attachments {
    /// `(entity, the bone it hung from)`, now parked under the skinned entity.
    hung: Vec<(u32, String)>,
    /// Each old bone's ancestor bones, nearest first.
    ancestors: BTreeMap<String, Vec<String>>,
}

impl Scene {
    /// Take `owner`'s skeleton down: its bone overrides and bodies go to the mesh's
    /// `BoneBinding` (which [`Scene::sync_skeleton`] applies), the entities hung
    /// from its bones move under `owner`, and the bones are despawned. Empty for an
    /// entity with no skin.
    pub(crate) fn take_skeleton(&mut self, owner: u32) -> Attachments {
        let Some((skin, bones)) = self
            .world
            .mesh(owner)
            .and_then(|m| Some((m.skin.clone()?, m.skeleton.bones.clone())))
        else {
            return Attachments::default();
        };
        // The documents `strip_bones` needs: the owner, its bones, and what hangs
        // from a bone (descendants of those keep their parent, so are not needed).
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
        strip_bones(&mut docs);

        let mut hung = Vec::new();
        for doc in &mut docs {
            if doc.id == owner {
                if let (Some(saved), Some(mut mesh)) = (&doc.mesh, self.world.mesh_mut(owner)) {
                    mesh.skeleton.overrides = saved.skeleton.overrides.clone();
                    mesh.skeleton.bodies = saved.skeleton.bodies.clone();
                }
            } else if let Some(bone) = doc.parent_bone.take() {
                hung.push((doc.id, bone));
            }
        }
        for &(child, _) in &hung {
            let _ = self.set_parent(child, Some(owner));
        }
        self.despawn_skeleton(owner);
        if let Some(mut mesh) = self.world.mesh_mut(owner) {
            mesh.skeleton.bones.clear();
        }
        let ancestors = (0..skin.local_bind.len())
            .map(|slot| (skin.name(slot), ancestor_names(&skin, slot)))
            .collect();
        Attachments { hung, ancestors }
    }

    /// Build `owner`'s skeleton from its (new) rig and hang `attachments` back on
    /// it by bone name. Returns the bones the rig no longer has that something
    /// hung from, each warned about; those entities went to the nearest surviving
    /// ancestor bone, or stayed under `owner`.
    pub(crate) fn restore_skeleton(&mut self, owner: u32, attachments: Attachments) -> Vec<String> {
        self.sync_skeleton(owner);
        let mut missing = Vec::new();
        for (child, name) in attachments.hung {
            let chain = attachments.ancestors.get(&name).into_iter().flatten();
            let found = std::iter::once(&name)
                .chain(chain)
                .find_map(|bone| Some((bone, self.find_bone(owner, bone)?)));
            if let Some((_, bone)) = found {
                let _ = self.set_parent(child, Some(bone));
            }
            if found.is_some_and(|(bone, _)| *bone == name) {
                continue;
            }
            let landed = found.map_or_else(
                || format!("entity {owner}"),
                |(bone, _)| format!("bone '{bone}'"),
            );
            log::warn!(
                "[Asset] entity {child} hung from bone '{name}', which the re-imported \
                 skeleton of entity {owner} no longer has; it now hangs from {landed}."
            );
            missing.push(name);
        }
        missing
    }
}

/// The names of `slot`'s ancestor joints in `skin`, nearest first. A parent link
/// that does not point to an earlier slot ends the chain, as `bind_bones` treats
/// such a joint as a root.
fn ancestor_names(skin: &crate::asset::SkinData, slot: usize) -> Vec<String> {
    let mut chain = Vec::new();
    let mut at = slot;
    while let Some(p) = skin.parents.get(at).copied().flatten().filter(|&p| p < at) {
        chain.push(skin.name(p));
        at = p;
    }
    chain
}
