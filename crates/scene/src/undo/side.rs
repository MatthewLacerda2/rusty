//! What the despawn door drops of a destroyed entity outside its document (its lightmap
//! entry, its owned decals, the loaded scene a root belongs to) rides with the Undo step, and
//! comes back with the entity. Undo never changes a live entity's entry: a bake is no edit.

use std::collections::{BTreeMap, BTreeSet};

use crate::decal::Decal;
use crate::lighting::lightmap_set::LightmapEntry;
use crate::runtime::membership::{Member, FIRST_SCENE};
use crate::Scene;

#[derive(Debug, Default)]
pub(super) struct Side {
    lightmaps: BTreeMap<u32, LightmapEntry>,
    decals: Vec<Decal>,
    /// A root's loaded scene (`LoadedScenes`' tag) and its `DontDestroyOnLoad` mark.
    tags: BTreeMap<u32, u32>,
    persistent: BTreeSet<u32>,
}

impl Side {
    pub(super) fn of(scene: &Scene, ids: impl IntoIterator<Item = u32>) -> Self {
        let ids: BTreeSet<u32> = ids.into_iter().collect();
        // The entries a save writes, never a progressive bake's preview.
        let entry = |id: &u32| Some((*id, scene.lightmaps.saved().get(*id)?.clone()));
        let lightmaps = ids.iter().filter_map(entry).collect();
        let owned = |d: &&Decal| d.owner.is_some_and(|o| ids.contains(&o.entity));
        let decals = scene.decals.iter().filter(owned).cloned().collect();
        let mut tags = BTreeMap::new();
        let mut persistent = BTreeSet::new();
        let roots = ids
            .iter()
            .filter(|&&id| scene.world.parent_id(id).is_none());
        for &id in roots {
            note_member(&mut tags, &mut persistent, id, scene.member_of(id));
        }
        Self {
            lightmaps,
            decals,
            tags,
            persistent,
        }
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
        for (id, tag) in later.tags {
            self.tags.entry(id).or_insert(tag);
        }
        self.persistent.extend(later.persistent);
    }

    pub(super) fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        self.lightmaps.retain(|&id, _| keep(id));
        self.decals
            .retain(|d| d.owner.is_some_and(|o| keep(o.entity)));
        self.tags.retain(|&id, _| keep(id));
        self.persistent.retain(|&id| keep(id));
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
        let root = |id: u32| scene.world.contains(id) && scene.world.parent_id(id).is_none();
        let handles = scene.loaded.handles();
        let home = |tag: u32| handles.is_empty() || handles.contains(&tag);
        let tags: Vec<(u32, u32)> = (self.tags.into_iter())
            .filter(|&(id, tag)| root(id) && home(tag))
            .collect();
        let marks: Vec<u32> = self.persistent.into_iter().filter(|&id| root(id)).collect();
        for (id, tag) in tags {
            if scene.loaded.tag_of(id).is_none() {
                scene.loaded.tag(id, tag);
            }
        }
        scene.loaded.persistent.extend(marks);
    }
}

/// The resolved scene, not the tag; before any join, the one a join makes [`FIRST_SCENE`].
fn note_member(
    tags: &mut BTreeMap<u32, u32>,
    persistent: &mut BTreeSet<u32>,
    id: u32,
    member: Option<Member>,
) {
    match member {
        Some(Member::DontDestroyOnLoad) => drop(persistent.insert(id)),
        Some(Member::Scene(Some(handle))) => drop(tags.insert(id, handle)),
        Some(Member::Scene(None)) => drop(tags.insert(id, FIRST_SCENE)),
        None => {}
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

    /// A root parented under another entity drops its scene, `before`: Undo brings it back.
    pub(crate) fn keep_dropped_tag(&mut self, id: u32, before: Option<Member>) {
        if !self.world.journal_recording() {
            return;
        }
        let mut dropped = Side::default();
        note_member(&mut dropped.tags, &mut dropped.persistent, id, before);
        self.history.dropped.absorb(dropped);
    }
}
