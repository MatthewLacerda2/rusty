//! Saving and restoring what a designer authored on a skeleton (#453).
//!
//! Bones are never saved. A document keeps, per skinned entity, the bones whose
//! transform differs from the model's rest pose (`BoneBinding::overrides`), and
//! every entity parented under a bone, re-pointed at the skinned entity with the
//! bone's name (then its ancestor bones') in `parent_bone`, and the `Rigidbody` /
//! `Joint` a ragdoll put on a bone (`BoneBinding::bodies`, #466); any other id leaf
//! naming a bone keeps the bone by name too (`bone_refs`, #1264). All re-bind by
//! name on load.

use std::collections::BTreeMap;

use super::bone_refs::{repoint_refs, BoneIndex};
use super::hang::Hung;
use super::sync::rest_local;
use crate::asset::SkinData;
use crate::components::mesh::BoneBody;
use crate::components::{Entity, TransformComponent};

/// How far a bone may drift from its rest pose before it counts as an override.
pub(super) const OVERRIDE_EPS: f32 = 1.0e-5;

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
    let bone_of = index_bones(entities, &index);
    if bone_of.is_empty() {
        return;
    }
    // Before the bones are recorded, so one jointed to another skeleton's bone saves
    // that bone by name too.
    repoint_refs(entities, &bone_of);
    for i in 0..entities.len() {
        let Some(mesh) = &entities[i].mesh else {
            continue;
        };
        let Some(skin) = &mesh.skin else {
            continue;
        };
        let mut saved = SavedBones::default();
        let bones = &mesh.skeleton.bones;
        for (slot, bone) in bones.iter().enumerate() {
            if let Some(&b) = index.get(bone) {
                saved.record(&entities[b], skin, bones, slot);
            }
        }
        if let Some(mesh) = &mut entities[i].mesh {
            mesh.skeleton.overrides = saved.overrides;
            mesh.skeleton.bodies = saved.bodies;
        }
    }
    reparent_attachments(entities, &index, &bone_of);
    entities.retain(|e| !bone_of.contains_key(&e.id));
    for entity in entities.iter_mut() {
        entity.children.retain(|c| !bone_of.contains_key(c));
    }
}

/// Every bone document in `entities` (`index` finds them by id), with its skinned
/// entity and bone chain.
fn index_bones(entities: &[Entity], index: &BTreeMap<u32, usize>) -> BoneIndex {
    let mut bone_of = BoneIndex::new();
    for entity in entities {
        let Some(mesh) = &entity.mesh else {
            continue;
        };
        let Some(skin) = &mesh.skin else {
            continue;
        };
        for (slot, bone) in mesh.skeleton.bones.iter().enumerate() {
            if index.contains_key(bone) {
                bone_of.insert(*bone, (entity.id, bone_chain(skin, slot)));
            }
        }
    }
    bone_of
}

/// What one skeleton saves of its bones, by bone name.
#[derive(Default)]
struct SavedBones {
    overrides: BTreeMap<String, TransformComponent>,
    bodies: BTreeMap<String, BoneBody>,
}

impl SavedBones {
    /// Record bone `slot`'s document: its transform when it is off the rest
    /// pose, its physics, and a warning for any other component.
    fn record(&mut self, doc: &Entity, skin: &SkinData, bones: &[u32], slot: usize) {
        let name = skin.name(slot);
        if !doc
            .transform
            .approx_eq(&rest_local(skin, slot), OVERRIDE_EPS)
        {
            self.overrides.insert(name.clone(), doc.transform.clone());
        }
        let chain_of = |id: u32| Some(bone_chain(skin, bones.iter().position(|&b| b == id)?));
        if let Some(body) = bone_body(doc, chain_of) {
            self.bodies.insert(name.clone(), body);
        }
        if carries_components(doc) {
            log::warn!(
                "[Scene] bone '{name}' carries components that are not saved (bones are \
                 rebuilt from the model); put them on a child of the bone instead."
            );
        }
    }
}

/// `slot`'s joint name, then the names of its ancestor joints in `skin`, nearest
/// first: what an attachment saves, so a bone the model later drops hands it to
/// the nearest one left. A parent link that does not point to an earlier slot ends
/// the chain, as `bind_bones` treats such a joint as a root.
pub(super) fn bone_chain(skin: &SkinData, slot: usize) -> Vec<String> {
    let mut chain = vec![skin.name(slot)];
    let mut at = slot;
    while let Some(p) = skin.parents.get(at).copied().flatten().filter(|&p| p < at) {
        chain.push(skin.name(p));
        at = p;
    }
    chain
}

/// Point every non-bone child of a bone at the bone's skinned entity instead,
/// remembering the bone by name.
fn reparent_attachments(
    entities: &mut [Entity],
    index: &BTreeMap<u32, usize>,
    bone_of: &BoneIndex,
) {
    let mut moved: Vec<(u32, u32)> = Vec::new(); // (attachment, skinned entity)
    for entity in entities.iter_mut() {
        if bone_of.contains_key(&entity.id) {
            continue;
        }
        let Some((owner, chain)) = entity.parent_id.and_then(|p| bone_of.get(&p)) else {
            continue;
        };
        entity.parent_id = Some(*owner);
        entity.parent_bone = chain.clone();
        moved.push((entity.id, *owner));
    }
    for (child, owner) in moved {
        if let Some(&i) = index.get(&owner) {
            entities[i].children.push(child);
        }
    }
}

/// A bone's `Rigidbody` and `Joint`, with a joint to another bone of the same
/// skeleton (`chain_of` resolves it) saved by that bone's chain, and one to another
/// skeleton's bone by its `bone_refs`. A joint to any other entity keeps its id.
/// `None` for a bone with neither.
fn bone_body(doc: &Entity, chain_of: impl Fn(u32) -> Option<Vec<String>>) -> Option<BoneBody> {
    if doc.rigidbody.is_none() && doc.joint.is_none() {
        return None;
    }
    let mut joint = doc.joint.clone();
    let connected_bone = joint
        .as_mut()
        .and_then(|j| {
            let chain = chain_of(j.connected_body?)?;
            j.connected_body = None;
            Some(chain)
        })
        .unwrap_or_default();
    let bone_refs = doc
        .bone_refs
        .iter()
        .filter(|(pointer, _)| pointer.starts_with("/joint/"))
        .map(|(p, chain)| (p.clone(), chain.clone()))
        .collect();
    Some(BoneBody {
        rigidbody: doc.rigidbody.clone(),
        joint,
        connected_bone,
        bone_refs,
    })
}

/// Whether a bone's document holds anything beyond what every bone has (name,
/// transform, hierarchy, flags, and the physics `bone_body` keeps) — a component
/// or a script.
pub(super) fn carries_components(doc: &Entity) -> bool {
    let mut bare = Entity::new(doc.id, doc.name.clone());
    bare.rigidbody = doc.rigidbody.clone();
    bare.joint = doc.joint.clone();
    bare.bone_refs = doc.bone_refs.clone();
    bare.active = doc.active;
    bare.is_static = doc.is_static;
    bare.layer = doc.layer;
    bare.transform = doc.transform.clone();
    bare.parent_id = doc.parent_id;
    bare.children = doc.children.clone();
    serde_json::to_value(&bare).ok() != serde_json::to_value(doc).ok()
}

/// Take the `parent_bone` marks out of a document about to be inserted, for
/// [`Scene::hang_on_skeletons`](crate::scene::Scene::hang_on_skeletons) once the skeletons exist.
pub fn take_bone_parents(entities: &mut [Entity]) -> Vec<Hung> {
    entities
        .iter_mut()
        .filter(|e| !e.parent_bone.is_empty())
        .map(|e| (e.id, std::mem::take(&mut e.parent_bone)))
        .collect()
}
