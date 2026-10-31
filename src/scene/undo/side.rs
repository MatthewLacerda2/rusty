//! src/scene/undo/side.rs — a destroyed entity's side tables ride with its Undo
//! record (#1631, #1649).
//!
//! The scene's one despawn door drops what `Scene` keeps keyed by a destroyed
//! entity outside its document: its lightmap entry (the last bake's output for
//! it) and the decals it owns. The scene state inventory classes both
//! `Keyed::Restored` (`scene::state`). So while an edit is recorded, what the
//! door drops is kept beside the step, and an Undo that brings the entity back
//! puts it back: the wall comes back lit and scorched, and the next save matches
//! the one before the destroy. Undo never changes a live entity's entry (a bake
//! is no edit).

use std::collections::{BTreeMap, BTreeSet};

use crate::scene::decal::Decal;
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::Scene;

/// What a step's destroyed entities held in the scene's side tables.
#[derive(Debug, Default)]
pub(super) struct Side {
    /// Lightmap entries by entity: each one's slot in the entry list and its value.
    lightmaps: BTreeMap<u32, (usize, LightmapEntry)>,
    /// The decals they owned, oldest first.
    decals: Vec<Decal>,
}

impl Side {
    /// The live side entries of `ids`: their lightmap entries, with their slots,
    /// and the decals they own.
    pub(super) fn of(scene: &Scene, ids: impl IntoIterator<Item = u32>) -> Self {
        let ids: BTreeSet<u32> = ids.into_iter().collect();
        let list = &scene.lightmaps.entries;
        let lightmaps = list
            .iter()
            .enumerate()
            .filter(|(_, e)| ids.contains(&e.entity))
            .map(|(slot, e)| (e.entity, (slot, e.clone())))
            .collect();
        let owned = |d: &&Decal| d.owner.is_some_and(|o| ids.contains(&o.entity));
        let decals = scene.decals.iter().filter(owned).cloned().collect();
        Self { lightmaps, decals }
    }

    /// Fold `later` in; what this one already holds is older, so it wins.
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

    /// Keep only what belongs to the entities `keep` accepts.
    pub(super) fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        self.lightmaps.retain(|&id, _| keep(id));
        self.decals
            .retain(|d| d.owner.is_some_and(|o| keep(o.entity)));
    }

    /// Put back what belongs to a live entity and is missing: each lightmap entry
    /// at its old slot (lowest first, so the list's order comes back as it was),
    /// each decal at its place in the blend order.
    pub(super) fn put_back(self, scene: &mut Scene) {
        let live = |id: u32| scene.world.contains(id);
        let mut missing: Vec<(usize, LightmapEntry)> = self
            .lightmaps
            .into_iter()
            .filter(|(id, _)| live(*id) && scene.lightmaps.get(*id).is_none())
            .map(|(_, slotted)| slotted)
            .collect();
        let decals: Vec<Decal> = self
            .decals
            .into_iter()
            .filter(|d| d.owner.is_some_and(|o| live(o.entity)))
            .collect();
        missing.sort_by_key(|(slot, _)| *slot);
        let list = &mut scene.lightmaps.entries;
        for (slot, entry) in missing {
            list.insert(slot.min(list.len()), entry);
        }
        scene.decals.put_back(decals);
    }
}

impl Scene {
    /// Keep the side entries of `ids` the despawn door is about to drop (its
    /// lightmap entries, and `decals`, the ones it took out), when the drop is
    /// part of a recorded edit.
    pub(crate) fn keep_dropped(&mut self, ids: &BTreeSet<u32>, decals: Vec<Decal>) {
        if !self.world.journal_recording() {
            return;
        }
        let mut dropped = Side::of(self, ids.iter().copied());
        dropped.decals = decals;
        self.history.dropped.absorb(dropped);
    }
}
