//! Hanging attachments back on a rebuilt skeleton (#453, #1252).
//!
//! A load, a prefab stamp and a live re-import all end the same way: the bones are
//! rebuilt from the model, and every entity that hung from a bone goes back under
//! the bone of the same name. When the model no longer has that bone, one rule
//! holds for all three: the entity goes to the nearest ancestor bone the model
//! still has, or stays on the skinned entity, with a warning naming the bone —
//! Unity keeps an object parented to a removed bone rather than dropping it. A
//! generated hitbox is the exception: it belongs to its bone, so it goes with it.

use std::collections::BTreeSet;

use super::bone_refs::BoneRef;
use crate::scene::Scene;

/// An entity waiting to hang from a bone: its id, and its bone's name followed by
/// that bone's ancestors', nearest first (`Entity::parent_bone`).
pub type Hung = (u32, Vec<String>);

impl Scene {
    /// Build every skinned mesh's skeleton, then hang `pending` (parked under their
    /// skinned entities) back on it and re-bind `refs`, the id leaves saved against
    /// a bone by name (`bone_refs`): what a load or a prefab stamp ends with.
    pub fn hang_on_skeletons(&mut self, pending: Vec<Hung>, mut refs: Vec<BoneRef>) {
        let parked: BTreeSet<u32> = pending.iter().map(|(id, _)| *id).collect();
        for owner in self.world.ids_with_mesh() {
            refs.extend(self.sync_skeleton_past(owner, &parked));
        }
        self.attach_to_bones(pending);
        self.bind_bone_refs(refs);
    }

    /// Re-parent each pending entity under the first bone its chain names that the
    /// skeleton of the skinned entity it is parked under has, warning about each
    /// one whose own bone is gone.
    pub fn attach_to_bones(&mut self, pending: Vec<Hung>) {
        for (child, chain) in pending {
            let (Some(owner), Some(name)) = (self.world.parent_id(child), chain.first()) else {
                continue;
            };
            let found = chain
                .iter()
                .enumerate()
                .find_map(|(at, bone)| Some((at, bone, self.find_bone(owner, bone)?)));
            if let Some((0, _, bone)) = found {
                let _ = self.set_parent(child, Some(bone));
                continue;
            }
            if self.is_generated_hitbox(child) {
                self.destroy_entity(child);
                log::warn!(
                    "[Asset] the skeleton of entity {owner} no longer has bone '{name}'; \
                     its generated hitbox (entity {child}) was removed."
                );
                continue;
            }
            let landed = match found {
                Some((_, bone_name, bone)) => {
                    let _ = self.set_parent(child, Some(bone));
                    format!("bone '{bone_name}'")
                }
                None => format!("entity {owner}"),
            };
            log::warn!(
                "[Asset] entity {child} hung from bone '{name}', which the skeleton of \
                 entity {owner} no longer has; it now hangs from {landed}."
            );
        }
    }
}
