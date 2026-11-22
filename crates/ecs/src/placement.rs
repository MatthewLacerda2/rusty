//! Whole-entity writes, `order` kept sorted by `Core::seq`. Every subtree walk follows `children`,
//! so a placed link gets its missing side: a live parent lists the child, an absent one is awaited.

use rusty_components::Entity;

use super::bundle::{build_bundle, Core};
use super::world::World;

impl World {
    /// The order every `ids_with_*` walk yields `id` in, for a cache kept in it.
    pub fn seq(&self, id: u32) -> Option<u64> {
        self.component::<Core>(id).map(|c| c.seq)
    }

    pub(super) fn place(&mut self, entity: Entity) {
        let seq = self.next_seq;
        self.place_at(entity, seq, self.order.len());
    }

    fn place_at(&mut self, entity: Entity, seq: u64, slot: usize) {
        let (id, parent) = (entity.id, entity.parent_id);
        self.next_seq = self.next_seq.max(seq + 1);
        self.touch_entity(id, entity.is_static);
        let mut builder = build_bundle(entity, seq);
        // lint: allow(ecs-write) the funnel: `touch_entity` noted the placement above
        let handle = self.inner.spawn(builder.build());
        self.handles.insert(id, handle);
        self.stamp_carried(id);
        self.order.insert(slot, id);
        self.list_in_parent(id, parent);
        self.adopt_waiting(id);
        self.moved(id); // a carried document brings a stale box
    }

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

    fn await_parent(&mut self, child: u32, id: u32) {
        self.awaited.entry(id).or_default().insert(child);
    }

    fn adopt_waiting(&mut self, id: u32) {
        let Some(waiting) = self.awaited.remove(&id) else {
            return;
        };
        let mut adopted = false;
        for c in waiting {
            if self.parent_id(c) == Some(id) && !self.lists_child(id, c) {
                self.add_child(id, c);
                self.moved(c);
                adopted = true;
            }
        }
        if adopted {
            self.invalidate_activity_below(id);
            self.stamp_hierarchy_below(id);
        }
    }

    /// Back at its old place in the insertion order, so a save after an Undo of a destroy writes
    /// the same order.
    pub fn restore_entity(&mut self, entity: Entity, seq: u64) {
        if self.contains(entity.id) {
            self.overwrite(entity);
            return;
        }
        let seq_of = |w: &World, id: u32| w.component::<Core>(id).map_or(0, |c| c.seq);
        let slot = self.order.partition_point(|&id| seq_of(self, id) < seq);
        self.next_id = self.next_id.max(entity.id + 1);
        self.place_at(entity, seq, slot);
        self.invalidate_caches();
    }

    pub(crate) fn overwrite(&mut self, entity: Entity) -> bool {
        let id = entity.id;
        let Some(old_handle) = self.handles.get(id) else {
            return false;
        };
        // Same slot in `order`, so the same stamp: a fresh one would sort it to the back.
        let seq = match self.inner.get::<&Core>(old_handle) {
            Ok(core) => core.seq,
            Err(_) => return false,
        };
        self.stamp_hierarchy_below(id); // and after: the flag and parent may differ
        self.touch_entity(id, entity.is_static);
        // lint: allow(ecs-write) the funnel: `touch_entity` stamped the overwrite above
        let _ = self.inner.despawn(old_handle);
        let parent = entity.parent_id;
        let mut builder = build_bundle(entity, seq);
        // lint: allow(ecs-write) the same overwrite's respawn
        let new_handle = self.inner.spawn(builder.build());
        self.handles.insert(id, new_handle);
        self.stamp_carried(id);
        self.moved(id);
        if let Some(p) = parent.filter(|&p| !self.contains(p)) {
            self.await_parent(id, p);
        }
        self.invalidate_caches();
        self.stamp_hierarchy_below(id);
        true
    }
}
