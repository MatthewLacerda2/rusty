use std::collections::BTreeMap;

use super::persist::bone_chain;
use crate::Scene;
use rusty_components::{Entity, EntityRefs};

pub(super) type BoneIndex = BTreeMap<u32, (u32, Vec<String>)>;

#[derive(Clone, Debug, PartialEq)]
pub struct BoneRef {
    pub holder: u32,
    pub pointer: String,
    pub owner: u32,
    pub chain: Vec<String>,
}

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
    pub(super) fn bone_index(&self) -> BoneIndex {
        let mut index = BoneIndex::new();
        for owner in self.world.ids_with_mesh() {
            for (bone, chain) in self.bones_with_chains(owner) {
                index.insert(bone, (owner, chain));
            }
        }
        index
    }

    fn bones_with_chains(&self, owner: u32) -> Vec<(u32, Vec<String>)> {
        let Some(skin) = self.world.mesh(owner).and_then(|m| m.skin.clone()) else {
            return Vec::new();
        };
        let slots = self.live_slots(owner).into_iter();
        slots.map(|(s, b)| (b, bone_chain(&skin, s))).collect()
    }

    pub fn bone_ref(&self, id: u32) -> Option<(u32, Vec<String>)> {
        let owner = self.bone_owner(id)?;
        let chain = self
            .bones_with_chains(owner)
            .into_iter()
            .find(|(b, _)| *b == id)?;
        Some((owner, chain.1))
    }

    pub fn bone_doc_refs(&self, docs: &mut [Entity]) {
        repoint_refs(docs, &self.bone_index());
    }

    pub(super) fn park_bone_refs(
        &mut self,
        owner: u32,
        removed: &BTreeMap<u32, Vec<String>>,
    ) -> Vec<BoneRef> {
        let mut out = Vec::new();
        for holder in self.world.ref_holders() {
            if removed.contains_key(&holder) {
                continue;
            }
            self.world.visit_refs(holder, &mut |pointer, r| {
                if let Some(chain) = r.and_then(|b| removed.get(&b)) {
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

    pub fn bind_doc_bone_refs(&self, doc: &mut Entity, holder: u32) {
        let chains = std::mem::take(&mut doc.bone_refs);
        doc.visit_refs(&mut |pointer, r| {
            if let (Some(chain), Some(owner)) = (chains.get(pointer), *r) {
                *r = Some(self.resolve_bone(holder, owner, chain));
            }
        });
    }

    pub(super) fn resolve_bone(&self, holder: u32, owner: u32, chain: &[String]) -> u32 {
        let (bone, landed) = self.resolve_chain(owner, chain);
        if let Some(landed) = landed {
            log::warn!(
                "[Asset] entity {holder} referred to bone '{}', which the skeleton of \
                 entity {owner} no longer has; it now refers to {landed}.",
                chain.first().map_or("", String::as_str)
            );
        }
        bone
    }

    pub(super) fn resolve_chain(&self, owner: u32, chain: &[String]) -> (u32, Option<String>) {
        let found = chain
            .iter()
            .enumerate()
            .find_map(|(at, name)| Some((at, name, self.live_bone(owner, name)?)));
        match found {
            Some((0, _, bone)) => (bone, None),
            Some((_, name, bone)) => (bone, Some(format!("bone '{name}'"))),
            None => (owner, Some(format!("entity {owner}"))),
        }
    }
}
