//! src/ecs/core.rs — the identity-core facade + mandatory components (#344, #345).
//!
//! Access to the facts every GameObject has — the slim "core component" of
//! #343's target model (`active`, `is_static`, `layer`, hierarchy links) —
//! plus the mandatory `name`, `Transform`, and scripts list, each its own
//! hecs column (#345), and the few whole-bundle *document* verbs
//! serialization and prefab propagation need.
//!
//! An owned `Entity` VALUE is a *document* (the serialized scene shape, kept
//! as the on-disk format): constructing one, serializing one, or diffing two
//! is fine anywhere. What consumers must not do is reach into live storage
//! directly — that lives in `world.rs`'s generic column accessors, used only
//! from this module and `access.rs`.
//!
//! Allowed deps: hecs, components.

use crate::components::{
    AnimatorComponent, Entity, MaterialAsset, MeshComponent, ScriptComponent, TransformComponent,
};

use super::access::{CompMut, CompRef};
use super::world::Core;
use super::world::World;

impl World {
    /// The mandatory Transform. `None` only when the entity does not exist.
    pub fn transform(&self, id: u32) -> Option<CompRef<'_, TransformComponent>> {
        self.component::<TransformComponent>(id).map(CompRef::new)
    }

    pub fn transform_mut(&mut self, id: u32) -> Option<CompMut<'_, TransformComponent>> {
        self.component_mut::<TransformComponent>(id)
            .map(CompMut::new)
    }

    /// The entity's script attachments (possibly empty; many per object is legal).
    pub fn scripts(&self, id: u32) -> Option<CompRef<'_, Vec<ScriptComponent>>> {
        self.component::<Vec<ScriptComponent>>(id).map(CompRef::new)
    }

    pub fn scripts_mut(&mut self, id: u32) -> Option<CompMut<'_, Vec<ScriptComponent>>> {
        self.component_mut::<Vec<ScriptComponent>>(id)
            .map(CompMut::new)
    }

    pub fn name(&self, id: u32) -> Option<CompRef<'_, String>> {
        self.component::<String>(id).map(CompRef::new)
    }

    pub fn set_name(&mut self, id: u32, name: String) -> bool {
        self.set_component(id, Some(name))
    }

    /// Whether the entity exists AND is active (`false` for a dead id).
    pub fn is_active(&self, id: u32) -> bool {
        self.component::<Core>(id).is_some_and(|c| c.active)
    }

    pub fn set_active(&mut self, id: u32, active: bool) -> bool {
        self.component_mut::<Core>(id)
            .map(|mut c| c.active = active)
            .is_some()
    }

    pub fn is_static(&self, id: u32) -> bool {
        self.component::<Core>(id).is_some_and(|c| c.is_static)
    }

    pub fn set_static(&mut self, id: u32, is_static: bool) -> bool {
        self.component_mut::<Core>(id)
            .map(|mut c| c.is_static = is_static)
            .is_some()
    }

    /// The entity's layer index (`0`, the default layer, for a dead id).
    pub fn layer(&self, id: u32) -> u8 {
        self.component::<Core>(id).map(|c| c.layer).unwrap_or(0)
    }

    pub fn set_layer(&mut self, id: u32, layer: u8) -> bool {
        self.component_mut::<Core>(id)
            .map(|mut c| c.layer = layer)
            .is_some()
    }

    pub fn parent_id(&self, id: u32) -> Option<u32> {
        self.component::<Core>(id).and_then(|c| c.parent_id)
    }

    /// Write the raw parent link. Hierarchy INVARIANTS (cycle checks, the
    /// parent's `children` list) are `Scene::set_parent`'s job; this is the
    /// storage-level write it and scene rehydration route through.
    pub fn set_parent_id(&mut self, id: u32, parent: Option<u32>) -> bool {
        self.component_mut::<Core>(id)
            .map(|mut c| c.parent_id = parent)
            .is_some()
    }

    /// The entity's child ids (cloned; empty for a dead id).
    pub fn children(&self, id: u32) -> Vec<u32> {
        self.component::<Core>(id)
            .map(|c| c.children.clone())
            .unwrap_or_default()
    }

    pub fn add_child(&mut self, parent: u32, child: u32) -> bool {
        self.component_mut::<Core>(parent)
            .map(|mut c| c.children.push(child))
            .is_some()
    }

    pub fn remove_child(&mut self, parent: u32, child: u32) -> bool {
        self.component_mut::<Core>(parent)
            .map(|mut c| c.children.retain(|&c| c != child))
            .is_some()
    }

    /// Take the transient legacy-material migration carrier, if one is staged.
    pub fn take_pending_material(&mut self, id: u32) -> Option<MaterialAsset> {
        self.take_component::<MaterialAsset>(id)
    }

    /// Stage a legacy-material migration carrier (the Add-menu's deferred
    /// library insert; folded into the scene's material library elsewhere).
    pub fn stage_pending_material(&mut self, id: u32, asset: MaterialAsset) -> bool {
        self.set_component(id, Some(asset))
    }

    /// The one sanctioned split borrow: mutate an entity's animator and (when
    /// present) its mesh together — the `animate` system's re-pose step. Now
    /// that they're independent hecs columns (#345) this is two native
    /// borrows under the hood. Returns `None` when the entity is missing or
    /// has no animator.
    pub fn with_animator_and_mesh_mut<R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut AnimatorComponent, Option<&mut MeshComponent>) -> R,
    ) -> Option<R> {
        self.with_pair_mut::<AnimatorComponent, MeshComponent, R>(id, f)
    }

    /// Assemble one live entity's columns into a document (serialization /
    /// prefab diffing).
    pub fn entity_document(&self, id: u32) -> Option<Entity> {
        let core = self.component::<Core>(id)?;
        let name = (*self.name(id)?).clone();
        let transform = (*self.transform(id)?).clone();
        let scripts = (*self.scripts(id)?).clone();
        Some(Entity {
            id: core.id,
            name,
            active: core.active,
            is_static: core.is_static,
            layer: core.layer,
            transform,
            mesh: self.mesh(id).map(|c| (*c).clone()),
            material: self.material(id).map(|c| (*c).clone()),
            pending_material: self.component::<MaterialAsset>(id).map(|c| (*c).clone()),
            scripts,
            animator: self.animator(id).map(|c| (*c).clone()),
            light: self.light(id).map(|c| (*c).clone()),
            collider: self.collider(id).map(|c| (*c).clone()),
            rigidbody: self.rigidbody(id).map(|c| (*c).clone()),
            nav_agent: self.nav_agent(id).map(|c| (*c).clone()),
            camera: self.camera(id).map(|c| (*c).clone()),
            visual_correction: self.visual_correction(id).map(|c| (*c).clone()),
            particles: self.particles(id).map(|c| (*c).clone()),
            audio: self.audio(id).map(|c| (*c).clone()),
            prefab_link: self.prefab_link(id).map(|c| (*c).clone()),
            parent_id: core.parent_id,
            children: core.children.clone(),
        })
    }

    /// Replace a live entity's whole set of columns from a document, in
    /// place — the prefab-propagation rebuild. Insertion order and the
    /// stable id keep their slots. Returns `false` when the entity does not
    /// exist.
    pub fn replace_entity(&mut self, entity: Entity) -> bool {
        if !self.contains(entity.id) {
            return false;
        }
        self.write_components(entity);
        true
    }
}
