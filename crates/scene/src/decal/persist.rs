use std::collections::{BTreeMap, BTreeSet};

use super::Decal;
use crate::Scene;
use rusty_ecs::HierarchyPath;

#[derive(Clone, Debug, PartialEq)]
pub struct CarriedDecal {
    decal: Decal,
    path: Option<HierarchyPath>,
}

impl Scene {
    pub fn take_persistent_decals(&mut self, survivors: &BTreeSet<u32>) -> Vec<CarriedDecal> {
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

    pub fn land_persistent_decals(&mut self, carried: Vec<CarriedDecal>) {
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
