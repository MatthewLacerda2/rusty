//! src/ecs/placement.rs — whole-entity writes: place a document as a new row,
//! overwrite a live row in place, put a removed row back in its old slot.
//!
//! Each lands one archetype placement through `bundle::build_bundle` (O(k), not
//! the O(k²) of N `insert_one` migrations), and each keeps `World::order` sorted
//! by `Core::seq`, the insertion order every narrow query recovers.

use crate::components::Entity;

use super::bundle::{build_bundle, Core};
use super::world::World;

impl World {
    /// Spawn a hecs entity carrying every column of `entity`, at the back of the
    /// insertion order under a fresh sequence stamp.
    pub(super) fn place(&mut self, entity: Entity) {
        let seq = self.next_seq;
        self.place_at(entity, seq, self.order.len());
    }

    /// Place `entity` with sequence stamp `seq` at index `slot` of the insertion
    /// order (the caller keeps `order` sorted by `seq`).
    fn place_at(&mut self, entity: Entity, seq: u64, slot: usize) {
        let id = entity.id;
        self.note(id);
        self.next_seq = self.next_seq.max(seq + 1);
        self.static_revision += u64::from(entity.is_static);
        let mut builder = build_bundle(entity, seq);
        let handle = self.inner.spawn(builder.build());
        self.handles.insert(id, handle);
        self.order.insert(slot, id);
        self.moved(id); // a carried document (a re-rooted load survivor) brings a stale box
    }

    /// Put a removed entity back as it was recorded (`World::record`): its id, and
    /// its old place in the insertion order, so a scene saved after an Undo of a
    /// destroy writes its entities in the order it did before (#1631). A live id
    /// is overwritten in place instead.
    pub fn restore_entity(&mut self, entity: Entity, seq: u64) {
        if self.contains(entity.id) {
            self.overwrite(entity);
            return;
        }
        let seq_of = |w: &World, id: u32| w.component::<Core>(id).map_or(0, |c| c.seq);
        let slot = self.order.partition_point(|&id| seq_of(self, id) < seq);
        self.next_id = self.next_id.max(entity.id + 1);
        self.place_at(entity, seq, slot);
        self.invalidate_caches(); // a child may already name this id as its parent
    }

    /// Overwrite a LIVE entity's whole column set with an `Entity` document in one
    /// placement (despawn + respawn under the same id, so no absent column survives
    /// stale) — `replace_entity`'s bridge. `false` when the entity does not exist.
    pub(in crate::ecs) fn overwrite(&mut self, entity: Entity) -> bool {
        let id = entity.id;
        let Some(old_handle) = self.handles.get(id) else {
            return false;
        };
        // The entity keeps its slot in insertion order (`self.order` is not
        // touched), so it must also keep its sequence stamp — a fresh one
        // would let narrow queries sort it to the back.
        let seq = match self.inner.get::<&Core>(old_handle) {
            Ok(core) => core.seq,
            Err(_) => return false,
        };
        self.note(id);
        let _ = self.inner.despawn(old_handle);
        let mut builder = build_bundle(entity, seq);
        let new_handle = self.inner.spawn(builder.build());
        self.handles.insert(id, new_handle);
        self.moved(id);
        self.invalidate_caches(); // its transform, parent and flag may all differ
        self.static_revision += 1;
        true
    }
}
