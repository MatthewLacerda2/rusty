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
    pub fn insert(&mut self, id: u32, handle: ColliderHandle) -> Option<ColliderHandle> {
        let was = self.map.insert(id, handle);
        self.generation += u64::from(was.is_none());
        was
    }

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
