use std::collections::BTreeSet;

use super::bone_refs::BoneRef;
use crate::{Despawn, Scene};

pub type Hung = (u32, Vec<String>);

impl Scene {
    pub fn hang_on_skeletons(&mut self, pending: Vec<Hung>, mut refs: Vec<BoneRef>) {
        let parked: BTreeSet<u32> = pending.iter().map(|(id, _)| *id).collect();
        for owner in self.world.ids_with_mesh() {
            refs.extend(self.sync_skeleton_past(owner, &parked));
        }
        self.attach_to_bones(pending, Despawn::Applying);
        self.bind_bone_refs(refs);
    }

    pub fn attach_to_bones(&mut self, pending: Vec<Hung>, why: Despawn) {
        for (child, chain) in pending {
            let (Some(owner), Some(name)) = (self.world.parent_id(child), chain.first()) else {
                continue;
            };
            let found = chain
                .iter()
                .enumerate()
                .find_map(|(at, bone)| Some((at, bone, self.live_bone(owner, bone)?)));
            if let Some((0, _, bone)) = found {
                let _ = self.set_parent(child, Some(bone));
                continue;
            }
            if self.is_generated_hitbox(child) {
                let doomed = self.destroy_set(child);
                self.despawn_batch(&doomed, why);
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
