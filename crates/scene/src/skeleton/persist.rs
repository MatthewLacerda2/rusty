//! What a document keeps of a skeleton, all re-bound by bone name on load.
use std::collections::BTreeMap;

use super::bone_refs::{repoint_refs, BoneIndex};
use super::hang::Hung;
use super::sync::rest_local;
use rusty_asset::SkinData;
use rusty_components::mesh::BoneBody;
use rusty_components::{Entity, TransformComponent};

/// How far a bone may drift from its rest pose before it counts as an override.
pub(super) const OVERRIDE_EPS: f32 = 1.0e-5;

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
    // Repointed before the bones are recorded, so a joint to another skeleton's bone saves
    // that bone by name too.
    repoint_refs(entities, &bone_of);
    for i in 0..entities.len() {
        let Some(mesh) = &entities[i].mesh else {
            continue;
        };
        let Some(skin) = &mesh.skin else {
            continue;
        };
        let owner = entities[i].id;
        let mut saved = SavedBones::default();
        for (slot, bone) in mesh.skeleton.bones.iter().enumerate() {
            let Some(bone) = bone.filter(|b| bone_of.contains_key(b)) else {
                continue;
            };
            saved.record(&entities[index[&bone]], skin, (owner, &bone_of), slot);
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

/// A bone a script moved out from under its skinned entity saves as an ordinary entity.
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
            match bone {
                Some(b) if doc_is_under(entities, index, *b, entity.id) => {
                    bone_of.insert(*b, (entity.id, bone_chain(skin, slot)));
                }
                _ => {}
            }
        }
    }
    bone_of
}

fn doc_is_under(entities: &[Entity], index: &BTreeMap<u32, usize>, id: u32, owner: u32) -> bool {
    let mut at = index.get(&id).and_then(|&i| entities[i].parent_id);
    for _ in 0..entities.len() {
        match at {
            Some(p) if p == owner => return true,
            Some(p) => at = index.get(&p).and_then(|&i| entities[i].parent_id),
            None => return false,
        }
    }
    false
}

#[derive(Default)]
struct SavedBones {
    overrides: BTreeMap<String, TransformComponent>,
    bodies: BTreeMap<String, BoneBody>,
}

impl SavedBones {
    fn record(
        &mut self,
        doc: &Entity,
        skin: &SkinData,
        (owner, bone_of): (u32, &BoneIndex),
        slot: usize,
    ) {
        let name = skin.name(slot);
        if !doc
            .transform
            .approx_eq(&rest_local(skin, slot), OVERRIDE_EPS)
        {
            self.overrides.insert(name.clone(), doc.transform.clone());
        }
        // A joint to a bone of this skeleton saves its chain; another skeleton's, its `bone_refs`.
        let chain_of = |id: u32| {
            let (of, chain) = bone_of.get(&id)?;
            (*of == owner).then(|| chain.clone())
        };
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

pub(super) fn bone_chain(skin: &SkinData, slot: usize) -> Vec<String> {
    let mut chain = vec![skin.name(slot)];
    let mut at = slot;
    // A parent link to no earlier slot ends the chain, as `bind_bones` makes such a joint a root.
    while let Some(p) = skin.parents.get(at).copied().flatten().filter(|&p| p < at) {
        chain.push(skin.name(p));
        at = p;
    }
    chain
}

fn reparent_attachments(
    entities: &mut [Entity],
    index: &BTreeMap<u32, usize>,
    bone_of: &BoneIndex,
) {
    let mut moved: Vec<(u32, u32)> = Vec::new();
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

pub fn take_bone_parents(entities: &mut [Entity]) -> Vec<Hung> {
    entities
        .iter_mut()
        .filter(|e| !e.parent_bone.is_empty())
        .map(|e| (e.id, std::mem::take(&mut e.parent_bone)))
        .collect()
}
