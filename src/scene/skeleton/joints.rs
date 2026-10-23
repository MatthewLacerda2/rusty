//! Joints to a bone from outside the skeleton (#1264).
//!
//! A prop on a rope tied to `spine` holds a `Joint` whose `connected_body` is a
//! bone. Bones get fresh ids on every load and every re-import, so that id is saved
//! by name, the way a child of a bone is (`Entity::parent_bone`): `connected_body`
//! names the skinned entity and `JointComponent::connected_bone` the bone, then
//! its ancestor bones, nearest first. Once the skeleton is back the joint is
//! re-bound to the first bone of that chain the skeleton still has, or, with none,
//! to the skinned entity, with a warning. A joint between two bones of one
//! skeleton is a ragdoll's, kept on the mesh's `BoneBinding` instead (`persist`).

use std::collections::BTreeMap;

use super::persist::bone_chain;
use crate::components::Entity;
use crate::scene::Scene;

/// Point every non-bone document's joint to a bone of `bone_of` (bone id → its
/// skinned entity and bone chain) at the skinned entity, the bone kept by name.
pub(super) fn repoint_joints(entities: &mut [Entity], bone_of: &BTreeMap<u32, (u32, Vec<String>)>) {
    for entity in entities.iter_mut() {
        if bone_of.contains_key(&entity.id) {
            continue;
        }
        let Some(joint) = entity.joint.as_mut() else {
            continue;
        };
        let Some((owner, chain)) = joint.connected_body.and_then(|b| bone_of.get(&b)) else {
            continue;
        };
        joint.connected_body = Some(*owner);
        joint.connected_bone = chain.clone();
    }
}

impl Scene {
    /// Before `owner`'s bones are despawned (a re-import), point every live joint
    /// to one of them from outside the skeleton at `owner`, the bone kept by name,
    /// for [`Scene::bind_bone_joints`] once the skeleton is rebuilt.
    pub(super) fn park_bone_joints(&mut self, owner: u32) {
        let Some((skin, bones)) = self
            .world
            .mesh(owner)
            .and_then(|m| Some((m.skin.clone()?, m.skeleton.bones.clone())))
        else {
            return;
        };
        for id in self.world.ids_with_joint() {
            if bones.contains(&id) {
                continue;
            }
            let Some(mut joint) = self.world.joint_mut(id) else {
                continue;
            };
            let Some(slot) = joint
                .connected_body
                .and_then(|b| bones.iter().position(|&x| x == b))
            else {
                continue;
            };
            joint.connected_body = Some(owner);
            joint.connected_bone = bone_chain(&skin, slot);
        }
    }

    /// Re-bind every joint saved against a bone by name (see the module docs) to
    /// that bone, now that the skeletons exist: what a load, a prefab stamp and a
    /// re-import end with.
    pub(crate) fn bind_bone_joints(&mut self) {
        for id in self.world.ids_with_joint() {
            let Some((owner, chain)) = self.world.joint_mut(id).and_then(|mut j| {
                let chain = std::mem::take(&mut j.connected_bone);
                Some((j.connected_body?, chain)).filter(|(_, c)| !c.is_empty())
            }) else {
                continue;
            };
            let found = chain
                .iter()
                .enumerate()
                .find_map(|(at, bone)| Some((at, bone, self.find_bone(owner, bone)?)));
            if let Some((_, _, bone)) = found {
                if let Some(mut j) = self.world.joint_mut(id) {
                    j.connected_body = Some(bone);
                }
            }
            let landed = match found {
                Some((0, _, _)) => continue,
                Some((_, name, _)) => format!("bone '{name}'"),
                None => format!("entity {owner}"),
            };
            log::warn!(
                "[Asset] entity {id}'s Joint was tied to bone '{}', which the skeleton of \
                 entity {owner} no longer has; it is now tied to {landed}.",
                chain[0]
            );
        }
    }
}
