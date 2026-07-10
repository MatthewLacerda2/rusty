//! src/ecs/world.rs — World wrapper over `hecs::World`.
//!
//! Storage flip (#345): each game object is now a hecs entity carrying real
//! per-component hecs columns — a slim mandatory [`Core`] (identity/hierarchy
//! facts), the mandatory `name: String` and `TransformComponent`, the mandatory
//! `Vec<ScriptComponent>`, and every optional first-class component attached
//! only when present. hecs itself enforces one-per-type storage. This wrapper
//! layers the engine's stable `u32` ids — the surface Lua scripts, the editor,
//! and the scene file all speak — plus a name lookup, fixing the old
//! `Player == 2` / `Enemy == 5` fragility.
//!
//! `spawn(name)` always attaches a Transform (via `Entity::new`): Transform is
//! the one mandatory component. Iteration is in insertion order so the legacy
//! `Vec<Entity>` semantics (physics pair ordering, render order) are preserved.
//!
//! The generic `component`/`component_mut`/`set_component`/`take_component`/
//! `has_component`/`with_pair_mut` helpers are the ONLY place raw hecs column
//! access happens; `access.rs` and `core.rs` build the typed, named surface on
//! top of them. `Entity` stays the on-disk *document* shape (see
//! `components::entity`) — `write_components`/`entity_document` are the
//! decompose/recompose bridge between the document and the columns.

use std::collections::HashMap;

use crate::components::Entity;

pub(in crate::ecs) use hecs::{Ref, RefMut};

/// The slim mandatory identity/hierarchy core every GameObject has (#343's
/// "core component"). Everything else — name, transform, scripts, and every
/// optional first-class component — is its own separately-attached hecs
/// column so narrow queries (#346) can skip entities that lack it.
pub(in crate::ecs) struct Core {
    pub id: u32,
    pub active: bool,
    pub is_static: bool,
    pub layer: u8,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

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
        let handle = self.inner.spawn(());
        self.handles.insert(id, handle);
        self.order.push(id);
        self.write_components(Entity::new(id, name));
        id
    }

    /// Insert a fully-formed `Entity` document (used when rehydrating a scene
    /// from disk), decomposing it into the mandatory + optional columns. The
    /// entity's `id` is honoured; `next_id` is advanced past it.
    pub fn insert_entity(&mut self, entity: Entity) {
        let id = entity.id;
        let handle = self.inner.spawn(());
        self.handles.insert(id, handle);
        self.order.push(id);
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        self.write_components(entity);
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

    /// Find the first *active* entity with this name, returning its stable id.
    /// Matches the legacy `Scene::find_entity_by_name` semantics.
    pub fn find_by_name(&self, name: &str) -> Option<u32> {
        self.order.iter().copied().find(|&id| {
            self.component::<String>(id).is_some_and(|n| *n == *name)
                && self.component::<Core>(id).is_some_and(|c| c.active)
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
        let &handle = self.handles.get(&id)?;
        self.inner.get::<&T>(handle).ok()
    }

    /// Exclusive borrow of one component column, by stable id.
    pub(in crate::ecs) fn component_mut<T: hecs::Component>(
        &mut self,
        id: u32,
    ) -> Option<RefMut<'_, T>> {
        let &handle = self.handles.get(&id)?;
        self.inner.get::<&mut T>(handle).ok()
    }

    pub(in crate::ecs) fn has_component<T: hecs::Component>(&self, id: u32) -> bool {
        self.component::<T>(id).is_some()
    }

    /// Attach (`Some`) or detach (`None`) a component column. Returns `false`
    /// when the entity does not exist.
    pub(in crate::ecs) fn set_component<T: hecs::Component>(
        &mut self,
        id: u32,
        value: Option<T>,
    ) -> bool {
        let Some(&handle) = self.handles.get(&id) else {
            return false;
        };
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
        let &handle = self.handles.get(&id)?;
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
        let &handle = self.handles.get(&id)?;
        let mut query = self
            .inner
            .query_one::<(&mut A, Option<&mut B>)>(handle)
            .ok()?;
        let (a, b) = query.get()?;
        Some(f(a, b))
    }

    /// Decompose an `Entity` document into its mandatory + optional columns
    /// and write them onto an already-registered handle — the shared bridge
    /// `spawn`/`insert_entity`/`replace_entity` all route through.
    pub(in crate::ecs) fn write_components(&mut self, entity: Entity) {
        let Entity {
            id,
            name,
            active,
            is_static,
            layer,
            transform,
            mesh,
            material,
            pending_material,
            scripts,
            animator,
            light,
            collider,
            rigidbody,
            nav_agent,
            camera,
            visual_correction,
            particles,
            audio,
            prefab_link,
            parent_id,
            children,
        } = entity;
        self.set_component(
            id,
            Some(Core {
                id,
                active,
                is_static,
                layer,
                parent_id,
                children,
            }),
        );
        self.set_component(id, Some(name));
        self.set_component(id, Some(transform));
        self.set_component(id, Some(scripts));
        self.set_component(id, mesh);
        self.set_component(id, material);
        self.set_component(id, pending_material);
        self.set_component(id, animator);
        self.set_component(id, light);
        self.set_component(id, collider);
        self.set_component(id, rigidbody);
        self.set_component(id, nav_agent);
        self.set_component(id, camera);
        self.set_component(id, visual_correction);
        self.set_component(id, particles);
        self.set_component(id, audio);
        self.set_component(id, prefab_link);
    }
}
