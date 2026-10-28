//! Decals across a scene load (#1518): `Scene::swap_in` takes the persistent ones
//! out before the outgoing World goes and lands them in the incoming one.
//!
//! An owned decal whose owner survives (`DontDestroyOnLoad`) keeps that owner, ids
//! being carried across. One whose owner does not is re-attached, in the same owner
//! space, to the incoming entity at the owner's hierarchy path
//! (`World::path_of` / `World::find_path`), so a mark on a crate comes back on the
//! crate wherever the incoming scene puts it; no entity at that path drops it. A
//! world decal stays where it is. Each keeps its id, age and fade, and they land
//! oldest first through `DecalSet::insert`, so the budgets apply and they stay
//! older than anything spawned after the load.

use std::collections::BTreeSet;

use super::Decal;
use crate::scene::Scene;

/// A persistent decal on its way across a load, with its owner's hierarchy path
/// when that owner is left behind.
#[derive(Clone, Debug, PartialEq)]
pub struct CarriedDecal {
    decal: Decal,
    path: Option<String>,
}

impl Scene {
    /// Drop every decal, returning the persistent ones to carry into the scene that
    /// replaces this one. `survivors` are the entities the load keeps; a decal whose
    /// owner is already gone is dropped.
    pub(crate) fn take_persistent_decals(
        &mut self,
        survivors: &BTreeSet<u32>,
    ) -> Vec<CarriedDecal> {
        let decals = self.decals.drain();
        let carry = |decal: Decal| {
            let path = match decal.owner {
                Some(o) if !survivors.contains(&o.entity) => Some(self.world.path_of(o.entity)?),
                _ => None,
            };
            Some(CarriedDecal { decal, path })
        };
        decals
            .into_iter()
            .filter(|d| d.persist)
            .filter_map(carry)
            .collect()
    }

    /// Land `carried` in the incoming World, oldest first: each left-behind owner is
    /// looked up by its path, and a decal with none to land on is dropped.
    pub(crate) fn land_persistent_decals(&mut self, carried: Vec<CarriedDecal>) {
        for CarriedDecal { mut decal, path } in carried {
            if let (Some(path), Some(owner)) = (path, decal.owner.as_mut()) {
                let Some(entity) = self.world.find_path(&path) else {
                    continue;
                };
                owner.entity = entity;
            }
            self.decals.insert(decal);
        }
    }
}

#[cfg(test)]
#[path = "persist_tests.rs"]
mod tests;
