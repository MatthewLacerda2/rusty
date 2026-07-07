//! src/ecs/core.rs — the identity-core facade + mandatory components (#344).
//!
//! Access to the facts every GameObject has — the slim "core component" of
//! #343's target model (`name`, `active`, `is_static`, `layer`, hierarchy
//! links) — plus the mandatory `Transform`, the scripts list, and the few
//! whole-bundle *document* verbs serialization and prefab propagation need.
//! Like `access.rs`, everything here reads the megastruct internally today so
//! #345 can flip the storage without touching consumers.
//!
//! An owned `Entity` VALUE is a *document* (the serialized scene shape, kept
//! by #345 as the on-disk format): constructing one, serializing one, or
//! diffing two is fine anywhere. What consumers must not do is project fields
//! out of *stored* entities — that projection lives in this module alone.
//!
//! Allowed deps: hecs, components.

use crate::components::{
    AnimatorComponent, Entity, MaterialAsset, MeshComponent, ScriptComponent, TransformComponent,
};

use super::access::{CompMut, CompRef};
use super::world::World;

impl World {
    /// The mandatory Transform. `None` only when the entity does not exist.
    pub fn transform(&self, id: u32) -> Option<CompRef<'_, TransformComponent>> {
        Some(CompRef::new(self.get(id)?, |e| &e.transform))
    }

    pub fn transform_mut(&mut self, id: u32) -> Option<CompMut<'_, TransformComponent>> {
        Some(CompMut::new(
            self.get_mut(id)?,
            |e| &e.transform,
            |e| &mut e.transform,
        ))
    }

    /// The entity's script attachments (possibly empty; many per object is legal).
    pub fn scripts(&self, id: u32) -> Option<CompRef<'_, Vec<ScriptComponent>>> {
        Some(CompRef::new(self.get(id)?, |e| &e.scripts))
    }

    pub fn scripts_mut(&mut self, id: u32) -> Option<CompMut<'_, Vec<ScriptComponent>>> {
        Some(CompMut::new(
            self.get_mut(id)?,
            |e| &e.scripts,
            |e| &mut e.scripts,
        ))
    }

    pub fn name(&self, id: u32) -> Option<CompRef<'_, String>> {
        Some(CompRef::new(self.get(id)?, |e| &e.name))
    }

    pub fn set_name(&mut self, id: u32, name: String) -> bool {
        self.get_mut(id).map(|mut e| e.name = name).is_some()
    }

    /// Whether the entity exists AND is active (`false` for a dead id).
    pub fn is_active(&self, id: u32) -> bool {
        self.get(id).is_some_and(|e| e.active)
    }

    pub fn set_active(&mut self, id: u32, active: bool) -> bool {
        self.get_mut(id).map(|mut e| e.active = active).is_some()
    }

    pub fn is_static(&self, id: u32) -> bool {
        self.get(id).is_some_and(|e| e.is_static)
    }

    pub fn set_static(&mut self, id: u32, is_static: bool) -> bool {
        self.get_mut(id)
            .map(|mut e| e.is_static = is_static)
            .is_some()
    }

    /// The entity's layer index (`0`, the default layer, for a dead id).
    pub fn layer(&self, id: u32) -> u8 {
        self.get(id).map(|e| e.layer).unwrap_or(0)
    }

    pub fn set_layer(&mut self, id: u32, layer: u8) -> bool {
        self.get_mut(id).map(|mut e| e.layer = layer).is_some()
    }

    pub fn parent_id(&self, id: u32) -> Option<u32> {
        self.get(id).and_then(|e| e.parent_id)
    }

    /// Write the raw parent link. Hierarchy INVARIANTS (cycle checks, the
    /// parent's `children` list) are `Scene::set_parent`'s job; this is the
    /// storage-level write it and scene rehydration route through.
    pub fn set_parent_id(&mut self, id: u32, parent: Option<u32>) -> bool {
        self.get_mut(id).map(|mut e| e.parent_id = parent).is_some()
    }

    /// The entity's child ids (cloned; empty for a dead id).
    pub fn children(&self, id: u32) -> Vec<u32> {
        self.get(id).map(|e| e.children.clone()).unwrap_or_default()
    }

    pub fn add_child(&mut self, parent: u32, child: u32) -> bool {
        self.get_mut(parent)
            .map(|mut e| e.children.push(child))
            .is_some()
    }

    pub fn remove_child(&mut self, parent: u32, child: u32) -> bool {
        self.get_mut(parent)
            .map(|mut e| e.children.retain(|&c| c != child))
            .is_some()
    }

    /// Take the transient legacy-material migration carrier, if one is staged.
    pub fn take_pending_material(&mut self, id: u32) -> Option<MaterialAsset> {
        self.get_mut(id)?.pending_material.take()
    }

    /// Stage a legacy-material migration carrier (the Add-menu's deferred
    /// library insert; folded into the scene's material library elsewhere).
    pub fn stage_pending_material(&mut self, id: u32, asset: MaterialAsset) -> bool {
        self.get_mut(id)
            .map(|mut e| e.pending_material = Some(asset))
            .is_some()
    }

    /// The one sanctioned split borrow: mutate an entity's animator and (when
    /// present) its mesh together — the `animate` system's re-pose step. After
    /// #345 these are independent hecs columns and this becomes two borrows.
    /// Returns `None` when the entity is missing or has no animator.
    pub fn with_animator_and_mesh_mut<R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut AnimatorComponent, Option<&mut MeshComponent>) -> R,
    ) -> Option<R> {
        let mut guard = self.get_mut(id)?;
        let Entity { animator, mesh, .. } = &mut *guard;
        let anim = animator.as_mut()?;
        Some(f(anim, mesh.as_mut()))
    }

    /// Clone one live entity out as a document (serialization / prefab diffing).
    pub fn entity_document(&self, id: u32) -> Option<Entity> {
        self.get(id).map(|e| (*e).clone())
    }

    /// Replace a live entity's whole bundle from a document, in place — the
    /// prefab-propagation rebuild. Insertion order and the stable id keep
    /// their slots. Returns `false` when the entity does not exist.
    pub fn replace_entity(&mut self, entity: Entity) -> bool {
        match self.get_mut(entity.id) {
            Some(mut e) => {
                *e = entity;
                true
            }
            None => false,
        }
    }
}
