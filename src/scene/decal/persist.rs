//! Decals across a scene load (#1518): `Scene::swap_in` takes the persistent ones
//! out before the outgoing World goes and lands them in the incoming one.
//!
//! An owned decal whose owner survives (`DontDestroyOnLoad`) keeps that owner, ids
//! being carried across. One whose owner does not is re-attached, in the same owner
//! space, to the incoming entity at the owner's hierarchy path
//! (`World::path_of` / `World::find_path`), so a mark on a crate comes back on the
//! crate wherever the incoming scene puts it; no entity at that path drops it. The
//! path is a `HierarchyPath`: each step a name and its ordinal among same-named
//! siblings, so the third of five `Crate`s comes back on the third. An owner its
//! path does not read back to in the outgoing World (a malformed hierarchy) is not
//! carried, rather than landing on the wrong entity. Paths are worked out once per
//! distinct owner on each side, so a load costs per owner, not per decal. A
//! world decal stays where it is. Each keeps its id, age and fade, and they land
//! oldest first through `DecalSet::insert`, so the budgets apply and they stay
//! older than anything spawned after the load.

use std::collections::{BTreeMap, BTreeSet};

use super::Decal;
use crate::ecs::HierarchyPath;
use crate::scene::Scene;

/// A persistent decal on its way across a load, with its owner's hierarchy path
/// when that owner is left behind.
#[derive(Clone, Debug, PartialEq)]
pub struct CarriedDecal {
    decal: Decal,
    path: Option<HierarchyPath>,
}

impl Scene {
    /// Drop every decal, returning the persistent ones to carry into the scene that
    /// replaces this one. `survivors` are the entities the load keeps; a decal whose
    /// owner is gone, or whose path does not name it, is dropped.
    pub(crate) fn take_persistent_decals(
        &mut self,
        survivors: &BTreeSet<u32>,
    ) -> Vec<CarriedDecal> {
        let world = &self.world;
        let mut paths: BTreeMap<u32, Option<HierarchyPath>> = BTreeMap::new();
        let mut path_to = |entity: u32| {
            let path = paths.entry(entity).or_insert_with(|| {
                let path = world.path_of(entity)?;
                (world.find_path(&path) == Some(entity)).then_some(path)
            });
            path.clone()
        };
        let mut carried = Vec::new();
        for decal in self.decals.drain().into_iter().filter(|d| d.persist) {
            let path = match decal.owner {
                Some(o) if !survivors.contains(&o.entity) => match path_to(o.entity) {
                    Some(path) => Some(path),
                    None => continue,
                },
                _ => None,
            };
            carried.push(CarriedDecal { decal, path });
        }
        carried
    }

    /// Land `carried` in the incoming World, oldest first: each left-behind owner is
    /// looked up by its path, and a decal with none to land on is dropped.
    pub(crate) fn land_persistent_decals(&mut self, carried: Vec<CarriedDecal>) {
        let mut found: BTreeMap<HierarchyPath, Option<u32>> = BTreeMap::new();
        for CarriedDecal { mut decal, path } in carried {
            if let (Some(path), Some(owner)) = (path, decal.owner.as_mut()) {
                let world = &self.world;
                let entity = found.entry(path).or_insert_with_key(|p| world.find_path(p));
                let Some(entity) = *entity else { continue };
                owner.entity = entity;
            }
            self.decals.insert(decal);
        }
    }
}

#[cfg(test)]
#[path = "persist_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "persist_paths_tests.rs"]
mod paths_tests;
