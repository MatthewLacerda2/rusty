//! References to a bone, kept by the bone's name (#1264).
//!
//! Any component id leaf can name a bone: a prop's Joint tied to `spine`, a
//! marker over `head`, one ragdoll's bone jointed to another skeleton's. Bones get
//! fresh ids on every load and every re-import, so such a leaf is saved the way a
//! child of a bone is (`Entity::parent_bone`): the leaf names the skinned entity, and
//! `Entity::bone_refs`, keyed by the leaf's pointer (`components::entity::refs`),
//! holds the bone's name, then its ancestor bones', nearest first. Once the skeleton
//! is back the leaf is re-bound to the first bone of that chain the skeleton still
//! has, or, with none, keeps the skinned entity, with a warning.
//!
//! One mechanism, every path: a save (`persist`'s `strip`), a load and a prefab stamp
//! ([`take_bone_refs`], then `Scene::hang_on_skeletons`), a live re-import
//! ([`Scene::park_bone_refs`]), a linked prefab's fresh baseline
//! ([`Scene::bone_doc_refs`] then [`Scene::bind_doc_bone_refs`]) and a prefab
//! apply-to-source ([`Scene::bone_ref`]). A reference between two bones of one
//! skeleton is a ragdoll's, kept on the mesh's `BoneBinding` instead (`persist`).

use std::collections::BTreeMap;

use super::persist::bone_chain;
use crate::components::Entity;
use crate::scene::Scene;

/// Bone id → its skinned entity and bone chain (the bone's name, then its
/// ancestors', nearest first).
pub(super) type BoneIndex = BTreeMap<u32, (u32, Vec<String>)>;

/// A component id leaf waiting to name a bone again: the entity holding it, the
/// leaf's pointer, the skinned entity the leaf names meanwhile, and the bone chain.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneRef {
    pub holder: u32,
    pub pointer: String,
    pub owner: u32,
    pub chain: Vec<String>,
}

/// Point every id leaf of `entities` that names a bone in `bone_of` at that bone's
/// skinned entity, the bone kept by name in `bone_refs`. A leaf of a bone naming a
/// bone of its own skeleton is left alone: that is the ragdoll's (`persist`).
pub(super) fn repoint_refs(entities: &mut [Entity], bone_of: &BoneIndex) {
    for entity in entities.iter_mut() {
        let own = bone_of.get(&entity.id).map(|(o, _)| *o);
        let mut found: Vec<(String, Vec<String>)> = Vec::new();
        entity.visit_refs(&mut |pointer, r| {
            let Some((owner, chain)) = r.and_then(|b| bone_of.get(&b)) else {
                return;
            };
            if own != Some(*owner) {
                *r = Some(*owner);
                found.push((pointer.to_string(), chain.clone()));
            }
        });
        entity.bone_refs.extend(found);
    }
}

/// Take the `bone_refs` out of documents about to be inserted, for
/// [`Scene::bind_bone_refs`] once the skeletons exist.
pub fn take_bone_refs(entities: &mut [Entity]) -> Vec<BoneRef> {
    let mut out = Vec::new();
    for entity in entities.iter_mut().filter(|e| !e.bone_refs.is_empty()) {
        let mut chains = std::mem::take(&mut entity.bone_refs);
        entity.each_ref(&mut |pointer, owner| {
            if let Some(chain) = chains.remove(pointer) {
                out.push(BoneRef {
                    holder: entity.id,
                    pointer: pointer.to_string(),
                    owner,
                    chain,
                });
            }
        });
    }
    out
}

impl Scene {
    /// Every bone of every skeleton, indexed for [`repoint_refs`].
    pub(super) fn bone_index(&self) -> BoneIndex {
        let mut index = BoneIndex::new();
        for owner in self.world.ids_with_mesh() {
            for (bone, chain) in self.bones_with_chains(owner) {
                index.insert(bone, (owner, chain));
            }
        }
        index
    }

    /// `owner`'s bones, each with its bone chain; empty without a skin.
    fn bones_with_chains(&self, owner: u32) -> Vec<(u32, Vec<String>)> {
        let Some((skin, bones)) = self
            .world
            .mesh(owner)
            .and_then(|m| Some((m.skin.clone()?, m.skeleton.bones.clone())))
        else {
            return Vec::new();
        };
        let live = |b: &u32| self.world.ancestors(*b).any(|a| a == owner);
        let slots = bones.into_iter().enumerate().filter(|(_, b)| live(b));
        slots.map(|(s, b)| (b, bone_chain(&skin, s))).collect()
    }

    /// The skinned entity bone `id` belongs to and its bone chain; `None` for an
    /// entity that is not a bone.
    pub fn bone_ref(&self, id: u32) -> Option<(u32, Vec<String>)> {
        let owner = self.bone_owner(id)?;
        let chain = self
            .bones_with_chains(owner)
            .into_iter()
            .find(|(b, _)| *b == id)?;
        Some((owner, chain.1))
    }

    /// Point `docs`' leaves naming a live bone at its skinned entity, the bone kept
    /// in `bone_refs`: a document read from this scene leaving its skeleton behind.
    pub(crate) fn bone_doc_refs(&self, docs: &mut [Entity]) {
        repoint_refs(docs, &self.bone_index());
    }

    /// Before `owner`'s bones are despawned (a re-import), point every live leaf
    /// naming one of them from outside its skeleton at `owner`, returning them for
    /// [`Scene::bind_bone_refs`] once the skeleton is rebuilt.
    pub(super) fn park_bone_refs(&mut self, owner: u32) -> Vec<BoneRef> {
        let bones: BTreeMap<u32, Vec<String>> = self.bones_with_chains(owner).into_iter().collect();
        let mut out = Vec::new();
        for holder in self.world.ref_holders() {
            if bones.contains_key(&holder) {
                continue;
            }
            self.world.visit_refs(holder, &mut |pointer, r| {
                if let Some(chain) = r.and_then(|b| bones.get(&b)) {
                    *r = Some(owner);
                    let (pointer, chain) = (pointer.to_string(), chain.clone());
                    out.push(BoneRef {
                        holder,
                        pointer,
                        owner,
                        chain,
                    });
                }
            });
        }
        out
    }

    /// Re-bind each pending leaf to its bone by name, now that the skeletons exist:
    /// what a load, a prefab stamp and a re-import end with. A leaf changed since it
    /// was parked is left alone.
    pub(crate) fn bind_bone_refs(&mut self, pending: Vec<BoneRef>) {
        for p in pending {
            let bone = self.resolve_bone(p.holder, p.owner, &p.chain);
            self.world.visit_refs(p.holder, &mut |pointer, r| {
                if pointer == p.pointer && *r == Some(p.owner) {
                    *r = Some(bone);
                }
            });
        }
    }

    /// Bind a document's `bone_refs` against this scene's live skeletons, clearing
    /// them: a linked prefab's fresh baseline, in instance ids.
    pub(crate) fn bind_doc_bone_refs(&self, doc: &mut Entity) {
        let chains = std::mem::take(&mut doc.bone_refs);
        let holder = doc.id;
        doc.visit_refs(&mut |pointer, r| {
            if let (Some(chain), Some(owner)) = (chains.get(pointer), *r) {
                *r = Some(self.resolve_bone(holder, owner, chain));
            }
        });
    }

    /// The first bone of `chain` `owner`'s skeleton has, or `owner` itself; warns
    /// unless it is the chain's own bone.
    fn resolve_bone(&self, holder: u32, owner: u32, chain: &[String]) -> u32 {
        let found = chain
            .iter()
            .enumerate()
            .find_map(|(at, name)| Some((at, name, self.find_bone(owner, name)?)));
        let landed = match found {
            Some((0, _, bone)) => return bone,
            Some((_, name, _)) => format!("bone '{name}'"),
            None => format!("entity {owner}"),
        };
        log::warn!(
            "[Asset] entity {holder} referred to bone '{}', which the skeleton of entity \
             {owner} no longer has; it now refers to {landed}.",
            chain.first().map_or("", String::as_str)
        );
        found.map_or(owner, |(_, _, bone)| bone)
    }
}
