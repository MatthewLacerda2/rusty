use std::collections::{BTreeMap, BTreeSet};

use crate::decal::Decal;
use crate::lighting::lightmap_set::LightmapEntry;
use crate::Scene;

#[derive(Debug, Default)]
pub(super) struct Side {
    lightmaps: BTreeMap<u32, LightmapEntry>,
    decals: Vec<Decal>,
}

impl Side {
    pub(super) fn of(scene: &Scene, ids: impl IntoIterator<Item = u32>) -> Self {
        let ids: BTreeSet<u32> = ids.into_iter().collect();
        let entry = |id: &u32| Some((*id, scene.lightmaps.saved().get(*id)?.clone()));
        let lightmaps = ids.iter().filter_map(entry).collect();
        let owned = |d: &&Decal| d.owner.is_some_and(|o| ids.contains(&o.entity));
        let decals = scene.decals.iter().filter(owned).cloned().collect();
        Self { lightmaps, decals }
    }

    pub(super) fn absorb(&mut self, later: Side) {
        for (id, entry) in later.lightmaps {
            self.lightmaps.entry(id).or_insert(entry);
        }
        for decal in later.decals {
            if !self.decals.iter().any(|d| d.id == decal.id) {
                self.decals.push(decal);
            }
        }
    }

    pub(super) fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        self.lightmaps.retain(|&id, _| keep(id));
        self.decals
            .retain(|d| d.owner.is_some_and(|o| keep(o.entity)));
    }

    pub(super) fn put_back(self, scene: &mut Scene) {
        let live = |id: u32| scene.world.contains(id);
        let missing: Vec<LightmapEntry> = self
            .lightmaps
            .into_iter()
            .filter(|(id, _)| live(*id) && scene.lightmaps.saved().get(*id).is_none())
            .map(|(_, entry)| entry)
            .collect();
        let decals: Vec<Decal> = self
            .decals
            .into_iter()
            .filter(|d| d.owner.is_some_and(|o| live(o.entity)))
            .collect();
        scene.lightmaps.extend(missing);
        scene.decals.put_back(decals);
    }
}

impl Scene {
    pub(crate) fn keep_dropped(&mut self, ids: &BTreeSet<u32>, decals: Vec<Decal>) {
        if !self.world.journal_recording() {
            return;
        }
        let mut dropped = Side::of(self, ids.iter().copied());
        dropped.decals = decals;
        self.history.dropped.absorb(dropped);
    }
}
