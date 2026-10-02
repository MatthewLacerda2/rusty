//! Saving and restoring what a designer authored on a skeleton (#453).
//!
//! Bones are never saved. A document keeps, per skinned entity, the bones whose
//! transform differs from the model's rest pose (`BoneBinding::overrides`), and
//! every entity parented under a bone, re-pointed at the skinned entity with the
//! bone's name in `parent_bone`, and the `Rigidbody` / `Joint` a ragdoll put on a
//! bone (`BoneBinding::bodies`, #466). All re-bind by name on load.

use std::collections::BTreeMap;

use super::sync::rest_local;
use crate::components::mesh::BoneBody;
use crate::components::Entity;
use crate::scene::Scene;

/// How far a bone may drift from its rest pose before it counts as an override.
const OVERRIDE_EPS: f32 = 1.0e-5;

/// Take every skeleton out of `entities` (a document about to be saved or
/// extracted): record each skinned entity's bone overrides and bone bodies, move
/// each bone's non-bone children under the skinned entity with `parent_bone` set,
/// and drop the bone documents. A bone carrying any other component is warned
/// about: only its transform and physics are kept, so a component belongs on a
/// child of the bone.
pub fn strip_bones(entities: &mut Vec<Entity>) {
    let index: BTreeMap<u32, usize> = entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id, i))
        .collect();
    // bone id -> (skinned entity id, bone name)
    let mut bone_of: BTreeMap<u32, (u32, String)> = BTreeMap::new();
    for i in 0..entities.len() {
        let Some(mesh) = &entities[i].mesh else {
            continue;
        };
        let Some(skin) = &mesh.skin else {
            continue;
        };
        let mut overrides = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        let bones = &mesh.skeleton.bones;
        for (slot, bone) in bones.iter().enumerate() {
            let Some(&b) = index.get(bone) else {
                continue;
            };
            let name = skin.name(slot);
            let doc = &entities[b];
            if !doc
                .transform
                .approx_eq(&rest_local(skin, slot), OVERRIDE_EPS)
            {
                overrides.insert(name.clone(), doc.transform.clone());
            }
            if let Some(body) = bone_body(doc, |id| {
                let slot = bones.iter().position(|&b| b == id)?;
                Some(skin.name(slot))
            }) {
                bodies.insert(name.clone(), body);
            }
            if carries_components(doc) {
                log::warn!(
                    "[Scene] bone '{name}' carries components that are not saved (bones are \
                     rebuilt from the model); put them on a child of the bone instead."
                );
            }
            bone_of.insert(*bone, (entities[i].id, name));
        }
        if let Some(mesh) = &mut entities[i].mesh {
            mesh.skeleton.overrides = overrides;
            mesh.skeleton.bodies = bodies;
        }
    }
    if bone_of.is_empty() {
        return;
    }
    reparent_attachments(entities, &index, &bone_of);
    entities.retain(|e| !bone_of.contains_key(&e.id));
    for entity in entities.iter_mut() {
        entity.children.retain(|c| !bone_of.contains_key(c));
    }
}

/// Point every non-bone child of a bone at the bone's skinned entity instead,
/// remembering the bone by name.
fn reparent_attachments(
    entities: &mut [Entity],
    index: &BTreeMap<u32, usize>,
    bone_of: &BTreeMap<u32, (u32, String)>,
) {
    let mut moved: Vec<(u32, u32)> = Vec::new(); // (attachment, skinned entity)
    for entity in entities.iter_mut() {
        if bone_of.contains_key(&entity.id) {
            continue;
        }
        let Some((owner, name)) = entity.parent_id.and_then(|p| bone_of.get(&p)) else {
            continue;
        };
        entity.parent_id = Some(*owner);
        entity.parent_bone = Some(name.clone());
        moved.push((entity.id, *owner));
    }
    for (child, owner) in moved {
        if let Some(&i) = index.get(&owner) {
            entities[i].children.push(child);
        }
    }
}

/// A bone's `Rigidbody` and `Joint`, with a joint to another bone of the same
/// skeleton (`bone_name` resolves it) saved by that bone's name. A joint to any
/// other entity keeps its id. `None` for a bone with neither.
fn bone_body(doc: &Entity, bone_name: impl Fn(u32) -> Option<String>) -> Option<BoneBody> {
    if doc.rigidbody.is_none() && doc.joint.is_none() {
        return None;
    }
    let mut joint = doc.joint.clone();
    let connected_bone = joint.as_mut().and_then(|j| {
        let name = bone_name(j.connected_body?)?;
        j.connected_body = None;
        Some(name)
    });
    Some(BoneBody {
        rigidbody: doc.rigidbody.clone(),
        joint,
        connected_bone,
    })
}

/// Whether a bone's document holds anything beyond what every bone has (name,
/// transform, hierarchy, flags, and the physics `bone_body` keeps) — a component
/// or a script.
fn carries_components(doc: &Entity) -> bool {
    let mut bare = Entity::new(doc.id, doc.name.clone());
    bare.rigidbody = doc.rigidbody.clone();
    bare.joint = doc.joint.clone();
    bare.active = doc.active;
    bare.is_static = doc.is_static;
    bare.layer = doc.layer;
    bare.transform = doc.transform.clone();
    bare.parent_id = doc.parent_id;
    bare.children = doc.children.clone();
    serde_json::to_value(&bare).ok() != serde_json::to_value(doc).ok()
}

/// Take the `parent_bone` marks out of a document about to be inserted, as
/// `(entity, bone name)` pairs for [`Scene::attach_to_bones`] once the skeletons
/// exist.
pub fn take_bone_parents(entities: &mut [Entity]) -> Vec<(u32, String)> {
    entities
        .iter_mut()
        .filter_map(|e| Some((e.id, e.parent_bone.take()?)))
        .collect()
}

impl Scene {
    /// Re-parent each `(entity, bone name)` under that bone of the skinned entity
    /// it was saved under. A bone the skeleton no longer has leaves the entity
    /// under the skinned entity, with a warning.
    pub fn attach_to_bones(&mut self, pending: Vec<(u32, String)>) {
        for (child, name) in pending {
            let Some(owner) = self.world.parent_id(child) else {
                continue;
            };
            match self.find_bone(owner, &name) {
                Some(bone) => {
                    let _ = self.set_parent(child, Some(bone));
                }
                None => log::warn!(
                    "[Asset] entity {child} was attached to bone '{name}', which the skeleton \
                     of entity {owner} no longer has; it stays under entity {owner}."
                ),
            }
        }
    }
}
