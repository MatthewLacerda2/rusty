//! src/scene/undo/lightmaps.rs — a destroyed entity's lightmap entry rides with
//! its Undo record (#1631).
//!
//! The scene's one despawn door drops a destroyed entity's lightmap entry, and the
//! entry is the last bake's output for it, not part of its document. So while an
//! edit is recorded, a dropped entry is kept beside the step, with its slot in the
//! entry list, and an Undo that brings the entity back puts it back there: the
//! wall comes back lit and the next save matches the one before the destroy.
//! Undo never changes a live entity's entry (a bake is no edit).

use std::collections::{BTreeMap, BTreeSet};

use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::Scene;

/// Lightmap entries by entity: each one's slot in the entry list and its value.
pub(super) type Entries = BTreeMap<u32, (usize, LightmapEntry)>;

impl Scene {
    /// Keep the lightmap entries of `ids` the despawn door is about to drop, when
    /// the drop is part of a recorded edit.
    pub(crate) fn keep_dropped_lightmaps(&mut self, ids: &BTreeSet<u32>) {
        if !self.world.journal_recording() {
            return;
        }
        let dropped = entries_of(self, ids.iter().copied());
        for (id, entry) in dropped {
            self.history.dropped.entry(id).or_insert(entry);
        }
    }
}

/// The live lightmap entries of `ids`, with their slots.
pub(super) fn entries_of(scene: &Scene, ids: impl IntoIterator<Item = u32>) -> Entries {
    let ids: BTreeSet<u32> = ids.into_iter().collect();
    let list = &scene.lightmaps.entries;
    list.iter()
        .enumerate()
        .filter(|(_, e)| ids.contains(&e.entity))
        .map(|(slot, e)| (e.entity, (slot, e.clone())))
        .collect()
}

/// Put back the entry of every live entity of `entries` that has none, at its
/// old slot (lowest first, so the list's order comes back as it was).
pub(super) fn put_back(scene: &mut Scene, entries: Entries) {
    let mut missing: Vec<(usize, LightmapEntry)> = entries
        .into_iter()
        .filter(|(id, _)| scene.world.contains(*id) && scene.lightmaps.get(*id).is_none())
        .map(|(_, slotted)| slotted)
        .collect();
    missing.sort_by_key(|(slot, _)| *slot);
    let list = &mut scene.lightmaps.entries;
    for (slot, entry) in missing {
        list.insert(slot.min(list.len()), entry);
    }
}
