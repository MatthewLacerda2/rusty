//! crates/physics/src/collider_ids.rs — collider entity id -> its rapier collider
//! handle, with a count of the edits made to it.
//!
//! The count is what tells a reader that keeps something derived from the table
//! (the bone-hung colliders, `follow`) that it moved: every insert and every
//! remove that took an entry bumps it, so no rebuild path can change the table
//! and leave the count behind.

use std::ops::Deref;

use rapier3d::prelude::ColliderHandle;
use rusty_core::collections::Map;

/// `id_to_collider`: read as the map it wraps, written only through
/// [`ColliderIds::insert`] and [`ColliderIds::remove`].
#[derive(Default)]
pub(crate) struct ColliderIds {
    map: Map<u32, ColliderHandle>,
    generation: u64,
}

impl ColliderIds {
    /// Map `id` to `handle`; the count moves.
    pub fn insert(&mut self, id: u32, handle: ColliderHandle) -> Option<ColliderHandle> {
        self.generation += 1;
        self.map.insert(id, handle)
    }

    /// Drop `id`'s entry; the count moves when there was one.
    pub fn remove(&mut self, id: &u32) -> Option<ColliderHandle> {
        let was = self.map.remove(id);
        self.generation += u64::from(was.is_some());
        was
    }

    /// Edits made since the table was made: equal counts, equal table.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl Deref for ColliderIds {
    type Target = Map<u32, ColliderHandle>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}
