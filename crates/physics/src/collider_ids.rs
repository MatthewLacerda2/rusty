//! crates/physics/src/collider_ids.rs — collider entity id -> its rapier collider
//! handle, with a count of the ids it gained or lost.
//!
//! The count is what tells a reader that keeps something derived from the table
//! (the bone-hung colliders, `follow`) which ids have a handle: an insert of a new
//! id and a remove that took an entry bump it, so no rebuild path can change the
//! id set and leave the count behind. An insert that only replaces an id's handle
//! (a body plan rebuilt) moves nothing: readers look the handle up each time.

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
    /// Map `id` to `handle`; the count moves when `id` had no handle.
    pub fn insert(&mut self, id: u32, handle: ColliderHandle) -> Option<ColliderHandle> {
        let was = self.map.insert(id, handle);
        self.generation += u64::from(was.is_none());
        was
    }

    /// Drop `id`'s entry; the count moves when there was one.
    pub fn remove(&mut self, id: &u32) -> Option<ColliderHandle> {
        let was = self.map.remove(id);
        self.generation += u64::from(was.is_some());
        was
    }

    /// Ids gained or lost since the table was made: equal counts, equal id set.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_count_moves_with_the_id_set_only() {
        let handle = |n| ColliderHandle::from_raw_parts(n, 0);
        let mut ids = ColliderIds::default();
        ids.insert(7, handle(0));
        assert_eq!(ids.generation(), 1, "a new id");
        ids.insert(7, handle(1));
        assert_eq!(ids.generation(), 1, "a replaced handle keeps the id set");
        assert_eq!(ids[&7], handle(1));
        ids.remove(&8);
        assert_eq!(ids.generation(), 1, "removing a missing id changes nothing");
        ids.remove(&7);
        assert_eq!(ids.generation(), 2, "a lost id");
    }
}
