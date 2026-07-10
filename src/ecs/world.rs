//! src/ecs/world.rs — World wrapper over `hecs::World`.
//!
//! Each game object is one hecs entity that holds a single `Entity` bundle
//! component (see `components::entity`). hecs owns identity and storage
//! (generational handles); this wrapper layers on the engine's stable `u32` ids
//! — the surface Lua scripts, the editor, and the scene file all speak — plus a
//! name lookup, fixing the old `Player == 2` / `Enemy == 5` fragility.
//!
//! `spawn(name)` always attaches a Transform (via `Entity::new`): Transform is
//! the one mandatory component. Iteration is in insertion order so the legacy
//! `Vec<Entity>` semantics (physics pair ordering, render order) are preserved.
//!
//! Component access returns hecs borrow guards (`Ref`/`RefMut`) which deref to
//! `&Entity` / `&mut Entity`; callers that hand the bundle to a helper pass
//! `&*guard` / `&mut *guard`.

use std::collections::HashMap;

use crate::components::Entity;

pub(in crate::ecs) use hecs::{Ref, RefMut};

pub struct World {
    inner: hecs::World,
    /// Stable engine id -> hecs handle (generational).
    handles: HashMap<u32, hecs::Entity>,
    /// Insertion order of stable ids — preserves legacy `Vec<Entity>` ordering.
    order: Vec<u32>,
    /// Monotonic stable-id allocator (mirrors the legacy `next_entity_id`).
    next_id: u32,
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
            handles: HashMap::new(),
            order: Vec::new(),
            next_id: 1,
        }
    }

    /// The next stable id that `spawn` would hand out (legacy `next_entity_id`).
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
        self.next_id += 1;
        let handle = self.inner.spawn((Entity::new(id, name),));
        self.handles.insert(id, handle);
        self.order.push(id);
        id
    }

    /// Insert a fully-formed `Entity` bundle (used when rehydrating a scene from
    /// disk). The entity's `id` is honoured; `next_id` is advanced past it.
    pub fn insert_entity(&mut self, entity: Entity) {
        let id = entity.id;
        let handle = self.inner.spawn((entity,));
        self.handles.insert(id, handle);
        self.order.push(id);
        if id >= self.next_id {
            self.next_id = id + 1;
        }
    }

    /// Despawn an entity by stable id.
    pub fn despawn(&mut self, id: u32) {
        if let Some(handle) = self.handles.remove(&id) {
            let _ = self.inner.despawn(handle);
        }
        self.order.retain(|&e| e != id);
    }

    /// Remove all entities and reset the id allocator (used on scene load).
    pub fn clear(&mut self) {
        self.inner.clear();
        self.handles.clear();
        self.order.clear();
        self.next_id = 1;
    }

    pub fn contains(&self, id: u32) -> bool {
        self.handles.contains_key(&id)
    }

    /// Borrow an entity's bundle. The returned guard derefs to `&Entity`.
    /// Facade-internal (#344): consumers go through the typed accessors in
    /// `access.rs` / `core.rs`, never through the whole bundle.
    pub(in crate::ecs) fn get(&self, id: u32) -> Option<Ref<'_, Entity>> {
        let handle = *self.handles.get(&id)?;
        self.inner.get::<&Entity>(handle).ok()
    }

    /// Mutably borrow an entity's bundle. The returned guard derefs to
    /// `&mut Entity`. Facade-internal (#344), like [`World::get`].
    pub(in crate::ecs) fn get_mut(&mut self, id: u32) -> Option<RefMut<'_, Entity>> {
        let handle = *self.handles.get(&id)?;
        self.inner.get::<&mut Entity>(handle).ok()
    }

    /// Find the first *active* entity with this name, returning its stable id.
    /// Matches the legacy `Scene::find_entity_by_name` semantics.
    pub fn find_by_name(&self, name: &str) -> Option<u32> {
        self.order.iter().copied().find(|&id| {
            self.get(id)
                .map(|e| e.name == name && e.active)
                .unwrap_or(false)
        })
    }

    /// The stable ids in insertion order.
    pub fn ids(&self) -> &[u32] {
        &self.order
    }

    /// Collect all entities (cloned) in insertion order — used for
    /// serialization snapshots.
    pub fn collect_entities(&self) -> Vec<Entity> {
        self.order
            .iter()
            .filter_map(|&id| self.get(id).map(|e| (*e).clone()))
            .collect()
    }
}
