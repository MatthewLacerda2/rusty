use std::collections::{BTreeMap, BTreeSet};

use rusty_components::Entity;

use super::activity::ActivityStore;
use super::bundle::Core;
use super::changes::ChangeLogs;
use super::despawn::RefIndex;
use super::handles::Handles;
use super::journal::Journal;
use super::names::Names;
use super::world_space::{BoundsQueue, WorldStore};

#[doc(hidden)]
pub use super::walks;
pub(crate) use hecs::{Ref, RefMut};

pub struct World {
    pub(super) inner: hecs::World,
    pub(super) handles: Handles,
    pub(super) order: Vec<u32>,
    pub(super) next_id: u32,
    pub(super) next_seq: u64,
    pub(super) world_space: WorldStore,
    pub(super) bounds: BoundsQueue,
    pub(super) activity: ActivityStore,
    pub(super) changes: std::cell::RefCell<ChangeLogs>,
    pub(super) journal: Journal,
    pub(super) refs: Option<RefIndex>,
    pub(super) awaited: BTreeMap<u32, BTreeSet<u32>>,
    pub(super) names: Names,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            inner: hecs::World::new(),
            handles: Handles::default(),
            order: Vec::new(),
            next_id: 1,
            next_seq: 0,
            world_space: WorldStore::default(),
            bounds: BoundsQueue::default(),
            activity: ActivityStore::default(),
            changes: Default::default(),
            journal: Journal::default(),
            refs: None,
            awaited: BTreeMap::new(),
            names: Names::default(),
        }
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn bump_next_id(&mut self, value: u32) {
        if value > self.next_id {
            self.next_id = value;
        }
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn spawn(&mut self, name: String) -> u32 {
        let id = self.next_id;
        self.note(id);
        self.next_id += 1;
        self.place(Entity::new(id, name));
        id
    }

    pub fn insert_entity(&mut self, entity: Entity) {
        self.note(entity.id);
        if entity.id >= self.next_id {
            self.next_id = entity.id + 1;
        }
        let id = entity.id;
        self.place(entity);
        self.invalidate_caches();
        self.stamp_hierarchy_below(id);
    }

    pub(super) fn remove_rows(&mut self, ids: &std::collections::BTreeSet<u32>) {
        let mut slots: Vec<usize> = ids.iter().filter_map(|&id| self.order_slot(id)).collect();
        for &id in ids {
            self.touch_entity(id, self.is_static(id));
            self.stamp_pose_below(id);
            self.world_space.forget(id);
            self.activity.forget_dead(id);
            if let Some(handle) = self.handles.remove(id) {
                // lint: allow(ecs-write) the funnel: `touch_entity` stamped the removal above
                let _ = self.inner.despawn(handle);
            }
        }
        slots.sort_unstable();
        drop_slots(&mut self.order, &slots);
    }

    fn order_slot(&self, id: u32) -> Option<usize> {
        let seq_of = |e: u32| self.component::<Core>(e).map_or(0, |c| c.seq);
        let seq = self.component::<Core>(id)?.seq;
        let at = self.order.partition_point(|&e| seq_of(e) < seq);
        let sorted = self.order.get(at) == Some(&id);
        debug_assert!(sorted, "`order` lost its `Core::seq` sort at {id}");
        match sorted {
            true => Some(at),
            false => self.order.iter().position(|&e| e == id),
        }
    }

    pub fn clear(&mut self) {
        self.inner.clear(); // lint: allow(ecs-write) the World is emptied: `new_epoch` below
        self.handles.clear();
        self.order.clear();
        self.journal = Journal::default();
        self.changes.get_mut().new_epoch();
        self.next_id = 1;
        self.next_seq = 0;
        self.reset_caches();
        self.bounds = BoundsQueue::default();
        self.changes.get_mut().reset_scratch();
        self.awaited.clear();
    }

    pub fn contains(&self, id: u32) -> bool {
        self.handles.get(id).is_some()
    }

    pub fn ids(&self) -> &[u32] {
        &self.order
    }

    #[doc(hidden)]
    pub fn collect_entities(&self) -> Vec<Entity> {
        self.order
            .iter()
            .filter_map(|&id| self.entity_document(id))
            .collect()
    }

    pub(crate) fn component<T: hecs::Component>(&self, id: u32) -> Option<Ref<'_, T>> {
        walks::lookup(std::any::type_name::<T>());
        let handle = self.handles.get(id)?;
        self.inner.get::<&T>(handle).ok()
    }

    pub(crate) fn each_component<T: hecs::Component>(&self, mut visit: impl FnMut(&T)) {
        self.inner.query::<&T>().iter().for_each(|(_, v)| visit(v));
    }

    pub(crate) fn has_component<T: hecs::Component>(&self, id: u32) -> bool {
        self.component::<T>(id).is_some()
    }

    pub(crate) fn ids_with_component<T: hecs::Component>(&self) -> Vec<u32> {
        let mut matches: Vec<(u64, u32)> = Vec::new();
        self.each_core_with::<T>(|core| matches.push((core.seq, core.id)));
        matches.sort_unstable_by_key(|&(seq, _)| seq);
        matches.into_iter().map(|(_, id)| id).collect()
    }

    pub(crate) fn ids_with_component_where<T: hecs::Component>(
        &self,
        keep: impl Fn(&T) -> bool,
    ) -> Vec<u32> {
        walks::count(std::any::type_name::<T>());
        let mut matches: Vec<(u64, u32)> = self
            .inner
            .query::<(&Core, &T)>()
            .iter()
            .filter(|(_, (_, value))| keep(value))
            .map(|(_, (core, _))| (core.seq, core.id))
            .collect();
        matches.sort_unstable_by_key(|&(seq, _)| seq);
        matches.into_iter().map(|(_, id)| id).collect()
    }

    pub(crate) fn each_core_with<T: hecs::Component>(&self, visit: impl FnMut(&Core)) {
        walks::count(std::any::type_name::<T>());
        let mut query = self.inner.query::<&Core>().with::<&T>();
        query.iter().map(|(_, core)| core).for_each(visit);
    }

    pub(crate) fn with_pair<A: hecs::Component, B: hecs::Component, R>(
        &self,
        id: u32,
        f: impl FnOnce(&A, &B) -> R,
    ) -> Option<R> {
        let handle = self.handles.get(id)?;
        let mut query = self.inner.query_one::<(&A, &B)>(handle).ok()?;
        let (a, b) = query.get()?;
        Some(f(a, b))
    }
}

fn drop_slots(order: &mut Vec<u32>, slots: &[usize]) {
    let Some(&first) = slots.first() else {
        return;
    };
    let mut write = first;
    for (k, &slot) in slots.iter().enumerate() {
        let end = slots.get(k + 1).copied().unwrap_or(order.len());
        order.copy_within(slot + 1..end, write);
        write += end - slot - 1;
    }
    order.truncate(write);
}
