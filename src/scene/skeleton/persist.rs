//! Saving and restoring what a designer authored on a skeleton (#453).
//!
//! Bones are never saved. A document keeps, per skinned entity, the bones whose
//! transform differs from the model's rest pose (`BoneBinding::overrides`), and
//! every entity parented under a bone, re-pointed at the skinned entity with the
//! bone's name (then its ancestor bones') in `parent_bone`, and the `Rigidbody` /
//! `Joint` a ragdoll put on a bone (`BoneBinding::bodies`, #466). All re-bind by
//! name on load.

use std::collections::BTreeMap;

use super::hang::Hung;
use super::sync::rest_local;
use crate::asset::SkinData;
use crate::components::mesh::BoneBody;
use crate::components::{Entity, TransformComponent};

/// How far a bone may drift from its rest pose before it counts as an override.
const OVERRIDE_EPS: f32 = 1.0e-5;

/// Take every skeleton out of `entities` (a document about to be saved or
/// extracted): record each skinned entity's bone overrides and bone bodies, move
/// each bone's non-bone children under the skinned entity with `parent_bone` set,
/// and drop the bone documents. A bone carrying any other component is warned
/// about: only its transform and physics are kept, so a component belongs on a
/// child of the bone.
pub fn strip_bones(entities: &mut Vec<Entity>) {
    strip(entities, Why::Saved);
}

/// Why bones are being taken out of a document, for the warning about the
/// components a bone carried.
#[derive(Clone, Copy)]
pub(super) enum Why {
    /// A save or extract: the components are not written.
    Saved,
    /// A live re-import (#1252): the bones are despawned, and the components with them.
    Reimported,
}

/// [`strip_bones`], warning about a bone's other components as `why` says.
pub(super) fn strip(entities: &mut Vec<Entity>, why: Why) {
    let index: BTreeMap<u32, usize> = entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id, i))
        .collect();
    // bone id -> (skinned entity id, the bone's name then its ancestors')
    let mut bone_of: BTreeMap<u32, (u32, Vec<String>)> = BTreeMap::new();
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
            let Some(&b) = index.get(bone) else {
                continue;
            };
            saved.record(&entities[b], skin, bones, slot, why);
            bone_of.insert(*bone, (entities[i].id, bone_chain(skin, slot)));
        }
        if let Some(mesh) = &mut entities[i].mesh {
            mesh.skeleton.overrides = saved.overrides;
            mesh.skeleton.bodies = saved.bodies;
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

/// What one skeleton saves of its bones, by bone name.
#[derive(Default)]
struct SavedBones {
    overrides: BTreeMap<String, TransformComponent>,
    bodies: BTreeMap<String, BoneBody>,
}

impl SavedBones {
    /// Record bone `slot`'s document: its transform when it is off the rest
    /// pose, its physics, and a warning for any other component.
    fn record(&mut self, doc: &Entity, skin: &SkinData, bones: &[u32], slot: usize, why: Why) {
        let name = skin.name(slot);
        if !doc
            .transform
            .approx_eq(&rest_local(skin, slot), OVERRIDE_EPS)
        {
            self.overrides.insert(name.clone(), doc.transform.clone());
        }
        let bone_name = |id: u32| Some(skin.name(bones.iter().position(|&b| b == id)?));
        if let Some(body) = bone_body(doc, bone_name) {
            self.bodies.insert(name.clone(), body);
        }
        if !carries_components(doc) {
            return;
        }
        match why {
            Why::Saved => log::warn!(
                "[Scene] bone '{name}' carries components that are not saved (bones are \
                 rebuilt from the model); put them on a child of the bone instead."
            ),
            Why::Reimported => log::warn!(
                "[Asset] bone '{name}' carried components; re-importing its model rebuilt \
                 the bone and deleted them. Put them on a child of the bone instead."
            ),
        }
    }
}

/// `slot`'s joint name, then the names of its ancestor joints in `skin`, nearest
/// first: what an attachment saves, so a bone the model later drops hands it to
/// the nearest one left. A parent link that does not point to an earlier slot ends
/// the chain, as `bind_bones` treats such a joint as a root.
fn bone_chain(skin: &SkinData, slot: usize) -> Vec<String> {
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
    bone_of: &BTreeMap<u32, (u32, Vec<String>)>,
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

/// Take the `parent_bone` marks out of a document about to be inserted, for
/// [`Scene::hang_on_skeletons`](crate::scene::Scene::hang_on_skeletons) once the skeletons exist.
pub fn take_bone_parents(entities: &mut [Entity]) -> Vec<Hung> {
    entities
        .iter_mut()
        .filter(|e| !e.parent_bone.is_empty())
        .map(|e| (e.id, std::mem::take(&mut e.parent_bone)))
        .collect()
}
