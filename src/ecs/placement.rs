//! src/ecs/placement.rs — whole-entity writes: place a document as a new row,
//! overwrite a live row in place, put a removed row back in its old slot.
//!
//! Each lands one archetype placement through `bundle::build_bundle` (O(k), not
//! the O(k²) of N `insert_one` migrations), and each keeps `World::order` sorted
//! by `Core::seq`, the insertion order every narrow query recovers.
//!
//! **Both sides of a placed link** (#1805). A document names its parent, and the
//! subtree walks (activity, world matrices, the change logs' hierarchy stamps, a
//! despawn's re-rooting) follow the parent's `children`, so a placement writes the
//! side its document lacks. A live parent not listing the child lists it at once.
//! A parent that is not live yet (a scene or prefab file naming an id it lacks, an
//! Undo putting a child back before its parent) is waited for, one entry per
//! dangling link in `World::awaited`: when that id arrives, each waiting child that
//! still names it is listed, and the subtree's kept answers go dirty. A
//! well-formed file whose parent is saved after its children waits too, once per
//! load. A placement with no waiting child costs one map lookup.

use crate::components::Entity;

use super::bundle::{build_bundle, Core};
use super::world::World;

impl World {
    /// `id`'s insertion sequence: the order every `ids_with_*` walk yields it in
    /// (`None` for a dead id). A cache holding entities in that order keys on it
    /// (the retained render scene, #1706).
    pub fn seq(&self, id: u32) -> Option<u64> {
        self.component::<Core>(id).map(|c| c.seq)
    }

    /// Spawn a hecs entity carrying every column of `entity`, at the back of the
    /// insertion order under a fresh sequence stamp.
    pub(super) fn place(&mut self, entity: Entity) {
        let seq = self.next_seq;
        self.place_at(entity, seq, self.order.len());
    }

    /// Place `entity` with sequence stamp `seq` at index `slot` of the insertion
    /// order (the caller keeps `order` sorted by `seq`).
    fn place_at(&mut self, entity: Entity, seq: u64, slot: usize) {
        let (id, parent) = (entity.id, entity.parent_id);
        self.next_seq = self.next_seq.max(seq + 1);
        self.touch_entity(id, entity.is_static);
        let mut builder = build_bundle(entity, seq);
        // lint: allow(ecs-write) the funnel: `touch_entity` stamped the placement above
        let handle = self.inner.spawn(builder.build());
        self.handles.insert(id, handle);
        self.order.insert(slot, id);
        self.list_in_parent(id, parent);
        self.adopt_waiting(id);
        self.moved(id); // a carried document (a re-rooted load survivor) brings a stale box
    }

    /// Placed `id` names `parent`: a live one lists it, an absent one is waited for
    /// (module docs). An Undo that overwrites the parent's own document next then
    /// replaces this list with the recorded one, which names `id` already.
    fn list_in_parent(&mut self, id: u32, parent: Option<u32>) {
        match parent {
            Some(p) if p == id || self.lists_child(p, id) => {}
            Some(p) if self.contains(p) => {
                self.add_child(p, id);
            }
            Some(p) => self.await_parent(id, p),
            None => {}
        }
    }

    /// `id` is waited for by placed `child` (module docs).
    fn await_parent(&mut self, child: u32, id: u32) {
        self.awaited.entry(id).or_default().insert(child);
    }

    /// `id` arrived: each child placed naming it while it was absent, and naming it
    /// still, joins its `children` (module docs).
    fn adopt_waiting(&mut self, id: u32) {
        let Some(waiting) = self.awaited.remove(&id) else {
            return;
        };
        let mut adopted = false;
        for c in waiting {
            if self.parent_id(c) == Some(id) && !self.lists_child(id, c) {
                self.add_child(id, c);
                self.moved(c); // its world pose now hangs from `id`
                adopted = true;
            }
        }
        if adopted {
            self.invalidate_activity_below(id);
            self.stamp_hierarchy_below(id);
        }
    }

    /// Put a removed entity back as it was recorded (`World::record`): its id, and
    /// its old place in the insertion order, so a scene saved after an Undo of a
    /// destroy writes its entities in the order it did before (#1631). A live id
    /// is overwritten in place instead.
    /// It stamps what any placement or overwrite stamps; the Undo that calls it
    /// takes one [`World::rewind_changes`] for the whole step.
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
        self.stamp_hierarchy_below(id); // before and after: the flag and parent may differ
        self.touch_entity(id, entity.is_static);
        // lint: allow(ecs-write) the funnel: `touch_entity` stamped the overwrite above
        let _ = self.inner.despawn(old_handle);
        let parent = entity.parent_id;
        let mut builder = build_bundle(entity, seq);
        // lint: allow(ecs-write) the same overwrite's respawn
        let new_handle = self.inner.spawn(builder.build());
        self.handles.insert(id, new_handle);
        self.moved(id);
        if let Some(p) = parent.filter(|&p| !self.contains(p)) {
            self.await_parent(id, p); // a live parent's list is the caller's
        }
        self.invalidate_caches(); // its transform, parent and flag may all differ
        self.stamp_hierarchy_below(id);
        true
    }
}
