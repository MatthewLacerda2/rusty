//! src/ecs/world.rs — World wrapper over `hecs::World`.
//!
//! Storage flip (#345): each game object is now a hecs entity carrying real
//! per-component hecs columns — a slim mandatory `Core` (identity/hierarchy
//! facts), the mandatory `name: String` and `TransformComponent`, the mandatory
//! `Vec<ScriptComponent>`, and every optional first-class component attached
//! only when present. hecs itself enforces one-per-type storage. This wrapper
//! layers the engine's stable `u32` ids — the surface Lua scripts, the editor,
//! and the scene file all speak — plus a name lookup.
//!
//! `spawn(name)` always attaches a Transform (via `Entity::new`): Transform is
//! the one mandatory component. Iteration is in insertion order so the former
//! `Vec<Entity>` semantics (physics pair ordering, render order) are preserved.
//!
//! The generic helpers (`component`, `each_component`, `set_component`, …) are
//! the ONLY place raw hecs column access happens; `access.rs` and `core.rs` build
//! the typed surface on them. `Entity` stays the on-disk *document* shape:
//! `bundle::build_bundle` turns one into a single `EntityBuilder` placement (one
//! archetype move), and `core.rs`'s `entity_document` recomposes it.

use crate::components::Entity;

use super::activity::ActivityStore;
use super::bundle::Core;
use super::handles::Handles;
use super::journal::Journal;
use super::world_space::{BoundsQueue, WorldStore};

pub(in crate::ecs) use hecs::{Ref, RefMut};

pub struct World {
    pub(super) inner: hecs::World,
    /// Stable engine id -> hecs handle (generational), a dense table (#696).
    pub(super) handles: Handles,
    /// Insertion order of stable ids — preserves the former `Vec<Entity>` ordering.
    /// Sorted by `Core::seq`: `place` appends, `overwrite` keeps the slot.
    pub(super) order: Vec<u32>,
    /// Monotonic stable-id allocator (mirrors the former `next_entity_id`).
    pub(super) next_id: u32,
    /// Insertion-sequence allocator: `place` stamps `Core::seq`, so narrow queries sort.
    pub(super) next_seq: u64,
    /// Every entity's world matrix, dirty-flagged (`ecs::world_space`).
    pub(super) world_space: WorldStore,
    /// The moved entities whose colliders' AABBs are due a refresh (`ecs::world_space`).
    pub(super) bounds: BoundsQueue,
    /// Every entity's `activeInHierarchy`, kept (`ecs::activity`).
    pub(super) activity: ActivityStore,
    /// Bumped whenever the set of static entities may have changed (`World::static_revision`).
    pub(super) static_revision: u64,
    /// The edit-mode change journal the editor's Undo reads (`ecs::journal`, #1631).
    pub(super) journal: Journal,
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
            static_revision: 0,
            journal: Journal::default(),
        }
    }

    /// The next stable id that `spawn` would hand out (the former `next_entity_id`).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Ensure the allocator will not reuse ids below `value` (used when loading a
    /// saved scene whose `next_entity_id` ran ahead of its max live id).
    pub fn bump_next_id(&mut self, value: u32) {
        if value > self.next_id {
            self.next_id = value;
        }
    }

    /// Number of live entities.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Spawn an entity with the given name, returning its stable id. Always
    /// attaches a Transform (mandatory) via `Entity::new`.
    pub fn spawn(&mut self, name: String) -> u32 {
        let id = self.next_id;
        self.note(id); // before the allocator moves, so an Undo gives the id back
        self.next_id += 1;
        self.place(Entity::new(id, name));
        id
    }

    /// Insert a fully-formed `Entity` document (used when rehydrating a scene
    /// from disk), decomposing it into the mandatory + optional columns. The
    /// entity's `id` is honoured; `next_id` is advanced past it.
    pub fn insert_entity(&mut self, entity: Entity) {
        self.note(entity.id);
        if entity.id >= self.next_id {
            self.next_id = entity.id + 1;
        }
        self.place(entity);
        self.invalidate_caches(); // an orphan may already name this id as parent
    }

    /// Drop the rows of `ids`, whose references `World::despawn_all` cleared first.
    pub(super) fn remove_rows(&mut self, ids: &std::collections::BTreeSet<u32>) {
        for &id in ids {
            self.note(id);
            self.static_revision += u64::from(self.is_static(id));
            self.world_space.forget(id);
            if let Some(handle) = self.handles.remove(id) {
                let _ = self.inner.despawn(handle);
            }
        }
        self.order.retain(|e| !ids.contains(e));
        self.activity.invalidate_all();
    }

    /// Remove all entities and reset the id allocator (used on scene load).
    pub fn clear(&mut self) {
        self.inner.clear();
        self.handles.clear();
        self.order.clear();
        self.journal = Journal::default();
        self.next_id = 1;
        self.next_seq = 0;
        self.reset_caches();
        self.bounds = BoundsQueue::default();
    }

    pub fn contains(&self, id: u32) -> bool {
        self.handles.get(id).is_some()
    }

    /// Find the first entity with this name that is active in the hierarchy
    /// (Unity's `GameObject.Find`), returning its stable id.
    pub fn find_by_name(&self, name: &str) -> Option<u32> {
        self.order.iter().copied().find(|&id| {
            self.component::<String>(id).is_some_and(|n| *n == *name)
                && self.is_active_in_hierarchy(id)
        })
    }

    /// The stable ids in insertion order.
    pub fn ids(&self) -> &[u32] {
        &self.order
    }

    /// Collect all entities (assembled as documents) in insertion order — used
    /// for serialization snapshots.
    pub fn collect_entities(&self) -> Vec<Entity> {
        self.order
            .iter()
            .filter_map(|&id| self.entity_document(id))
            .collect()
    }

    /// Shared borrow of one component column, by stable id.
    pub(in crate::ecs) fn component<T: hecs::Component>(&self, id: u32) -> Option<Ref<'_, T>> {
        let handle = self.handles.get(id)?;
        self.inner.get::<&T>(handle).ok()
    }

    /// Exclusive borrow of one component column, by stable id.
    pub(in crate::ecs) fn component_mut<T: hecs::Component>(
        &mut self,
        id: u32,
    ) -> Option<RefMut<'_, T>> {
        let handle = self.handles.get(id)?;
        self.note(id);
        self.inner.get::<&mut T>(handle).ok()
    }

    /// Visit every `T`, in no set order and without allocating.
    pub(in crate::ecs) fn each_component<T: hecs::Component>(&self, mut visit: impl FnMut(&T)) {
        self.inner.query::<&T>().iter().for_each(|(_, v)| visit(v));
    }

    pub(in crate::ecs) fn has_component<T: hecs::Component>(&self, id: u32) -> bool {
        self.component::<T>(id).is_some()
    }

    /// Stable ids of the entities carrying component `T`, in insertion order —
    /// the narrow query (#346). Cost tracks the carrier population, not the
    /// scene: hecs yields only matching archetypes (O(k)), and the sort on
    /// `Core::seq` (O(k log k)) restores the insertion order hecs's
    /// per-archetype iteration doesn't provide.
    pub(in crate::ecs) fn ids_with_component<T: hecs::Component>(&self) -> Vec<u32> {
        let mut matches: Vec<(u64, u32)> = self
            .inner
            .query::<&Core>()
            .with::<&T>()
            .iter()
            .map(|(_, core)| (core.seq, core.id))
            .collect();
        matches.sort_unstable_by_key(|&(seq, _)| seq);
        matches.into_iter().map(|(_, id)| id).collect()
    }

    /// Attach (`Some`) or detach (`None`) a component column. Returns `false`
    /// when the entity does not exist.
    pub(in crate::ecs) fn set_component<T: hecs::Component>(
        &mut self,
        id: u32,
        value: Option<T>,
    ) -> bool {
        let Some(handle) = self.handles.get(id) else {
            return false;
        };
        self.note(id);
        match value {
            Some(v) => {
                let _ = self.inner.insert_one(handle, v);
            }
            None => {
                let _ = self.inner.remove_one::<T>(handle);
            }
        }
        true
    }

    /// Detach and return a component column (`None` when absent or the entity
    /// is dead).
    pub(in crate::ecs) fn take_component<T: hecs::Component>(&mut self, id: u32) -> Option<T> {
        let handle = self.handles.get(id)?;
        self.note(id);
        self.inner.remove_one::<T>(handle).ok()
    }

    /// Borrow two DIFFERENT component columns of the same entity mutably at
    /// once — sound because they're independent hecs columns. `None` when the
    /// entity is missing or lacks the mandatory `A`.
    pub(in crate::ecs) fn with_pair_mut<A: hecs::Component, B: hecs::Component, R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut A, Option<&mut B>) -> R,
    ) -> Option<R> {
        let handle = self.handles.get(id)?;
        self.note(id);
        let mut query = self
            .inner
            .query_one::<(&mut A, Option<&mut B>)>(handle)
            .ok()?;
        let (a, b) = query.get()?;
        Some(f(a, b))
    }

    /// Shared borrow of two columns of one entity under a single id lookup.
    pub(in crate::ecs) fn with_pair<A: hecs::Component, B: hecs::Component, R>(
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
