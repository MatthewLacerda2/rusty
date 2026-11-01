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

use std::collections::BTreeMap;

use crate::components::{
    AnimatorComponent, ColliderComponent, Entity, MaterialAsset, MeshComponent, ScriptComponent,
    TransformComponent,
};

use super::access::{CompMut, CompRef};
use super::bundle::Core;
use super::world::World;

impl World {
    /// The mandatory Transform. `None` only when the entity does not exist.
    pub fn transform(&self, id: u32) -> Option<CompRef<'_, TransformComponent>> {
        self.component::<TransformComponent>(id).map(CompRef::new)
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

    /// Unity's `activeSelf`: the entity's own `active` flag (`false` for a dead id).
    /// Only what shows or stores the flag itself reads it (the inspector checkbox,
    /// `Scene.IsActive`, the snapshot); every system asks
    /// [`World::is_active_in_hierarchy`] instead, and the `active-self` lint holds
    /// that (#1363).
    pub fn is_active_self(&self, id: u32) -> bool {
        self.component::<Core>(id).is_some_and(|c| c.active)
    }

    /// Unity's `activeInHierarchy`: the entity exists and it and every ancestor
    /// are active. An inactive parent hides and stops its whole subtree without
    /// touching the children's own flags (#1363). The one question every system
    /// asks before drawing, simulating or ticking an entity; answered once and kept
    /// until a flag or a parent link changes (`ecs::activity`, #1388).
    pub fn is_active_in_hierarchy(&self, id: u32) -> bool {
        self.contains(id) && self.activity.resolve(self, id)
    }

    pub fn set_active(&mut self, id: u32, active: bool) -> bool {
        let Some(mut core) = self.component_mut::<Core>(id) else {
            return false;
        };
        if std::mem::replace(&mut core.active, active) != active {
            drop(core);
            self.activity.invalidate_all();
            // The shadow sweeps skip inactive entities, so a static one going dark or
            // lit changes the static caster set. A toggle with no static in its subtree
            // (a muzzle flash, a pickup) leaves the static bakes alone.
            if self.self_and_descendants(id).any(|e| self.is_static(e)) {
                self.static_revision += 1;
            }
        }
        true
    }

    pub fn is_static(&self, id: u32) -> bool {
        self.component::<Core>(id).is_some_and(|c| c.is_static)
    }

    /// Unity's `GameObject.isStatic`, but live: physics, the shadow sweeps and probe
    /// lighting read it every frame. A change bumps [`World::static_revision`].
    pub fn set_static(&mut self, id: u32, is_static: bool) -> bool {
        let Some(mut core) = self.component_mut::<Core>(id) else {
            return false;
        };
        if std::mem::replace(&mut core.is_static, is_static) != is_static {
            drop(core);
            self.static_revision += 1;
        }
        true
    }

    /// Moves whenever the set of static entities may have changed: a flag flipped, a
    /// static entity (or an ancestor of one) activated or deactivated, a static entity
    /// placed or removed, a row overwritten, the World cleared. Whoever
    /// made the change (the inspector, a script in Play, a load), a cache of static
    /// geometry keyed on it (the renderer's static shadow bakes) goes stale (#1496).
    pub fn static_revision(&self) -> u64 {
        self.static_revision
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

    /// The local Transform matrix and the parent, in one lookup — the hot read
    /// of every hierarchy walk (the skin palette build).
    pub fn local_and_parent(&self, id: u32) -> Option<(glam::Mat4, Option<u32>)> {
        self.with_pair::<TransformComponent, Core, _>(id, |t, c| (t.to_matrix(), c.parent_id))
    }

    pub fn parent_id(&self, id: u32) -> Option<u32> {
        self.component::<Core>(id).and_then(|c| c.parent_id)
    }

    /// `id`'s parent, grandparent, … up to the root — every hierarchy walk upward
    /// goes through here (#939). At most [`World::len`] steps, so a malformed parent
    /// cycle ends the walk instead of hanging it; a parent id no longer in the world
    /// ends it too, as if `id`'s chain stopped there (#991).
    pub fn ancestors(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        let live_parent = |id: u32| self.parent_id(id).filter(|&p| self.contains(p));
        std::iter::successors(live_parent(id), move |&p| live_parent(p)).take(self.len())
    }

    /// `id`, then [`World::ancestors`].
    pub fn self_and_ancestors(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(id).chain(self.ancestors(id))
    }

    /// The entity whose physics body carries `id`: `id` itself or its nearest
    /// ancestor with a `Rigidbody` or a `CharacterController` (a body of its own,
    /// #451); `None` when nothing up the hierarchy has one. The one rule the
    /// physics body plan and a loaded joint's warning share.
    pub fn body_carrier(&self, id: u32) -> Option<u32> {
        self.self_and_ancestors(id)
            .find(|&c| self.has_rigidbody(c) || self.has_character_controller(c))
    }

    /// Write the raw parent link. Hierarchy INVARIANTS (cycle checks, the
    /// parent's `children` list) are `Scene::set_parent`'s job; this is the
    /// storage-level write it and scene rehydration route through.
    /// The subtree moves in world space, so its world matrices go dirty and its
    /// colliders' AABBs follow (`ecs::world_space`).
    pub fn set_parent_id(&mut self, id: u32, parent: Option<u32>) -> bool {
        let Some(mut core) = self.component_mut::<Core>(id) else {
            return false;
        };
        let relinked = std::mem::replace(&mut core.parent_id, parent) != parent;
        drop(core);
        self.moved(id);
        if relinked {
            self.activity.invalidate_all();
        }
        true
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

    /// Take the material staged for the scene's library, if one is staged.
    pub fn take_pending_material(&mut self, id: u32) -> Option<MaterialAsset> {
        self.take_component::<MaterialAsset>(id)
    }

    /// Stage a material for the scene's library (the Add-menu's deferred library
    /// insert; folded into the scene's material library elsewhere).
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

    /// Assemble one live entity's columns into a document (serialization).
    pub fn entity_document(&self, id: u32) -> Option<Entity> {
        self.document(id, MeshComponent::clone, ColliderComponent::clone)
    }

    /// [`World::entity_document`] with the mesh's reference and none of its
    /// geometry ([`MeshComponent::reference_only`]), and a collider's shared mesh
    /// without its geometry either: what a prefab diff reads.
    pub fn reference_document(&self, id: u32) -> Option<Entity> {
        self.document(
            id,
            MeshComponent::reference_only,
            ColliderComponent::without_geometry,
        )
    }

    /// The document of entity `id`, its mesh copied by `mesh` and its collider by
    /// `collider` (whose shared mesh may carry geometry, #1657).
    pub(super) fn document(
        &self,
        id: u32,
        mesh: fn(&MeshComponent) -> MeshComponent,
        collider: fn(&ColliderComponent) -> ColliderComponent,
    ) -> Option<Entity> {
        let core = self.component::<Core>(id)?;
        let name = (*self.name(id)?).clone();
        let transform = (*self.transform(id)?).clone();
        Some(Entity {
            id: core.id,
            name,
            active: core.active,
            is_static: core.is_static,
            layer: core.layer,
            transform,
            mesh: self.mesh(id).map(|c| mesh(&c)),
            material: self.material(id).map(|c| (*c).clone()),
            pending_material: self.component::<MaterialAsset>(id).map(|c| (*c).clone()),
            scripts: (*self.scripts(id)?).clone(),
            animator: self.animator(id).map(|c| (*c).clone()),
            light: self.light(id).map(|c| (*c).clone()),
            collider: self.collider(id).map(|c| collider(&c)),
            rigidbody: self.rigidbody(id).map(|c| (*c).clone()),
            nav_agent: self.nav_agent(id).map(|c| (*c).clone()),
            nav_obstacle: self.nav_obstacle(id).map(|c| (*c).clone()),
            offmesh_link: self.offmesh_link(id).map(|c| (*c).clone()),
            nav_modifier: self.nav_modifier(id).map(|c| (*c).clone()),
            camera: self.camera(id).map(|c| (*c).clone()),
            post_processing: self.post_processing(id).map(|c| (*c).clone()),
            particles: self.particles(id).map(|c| (*c).clone()),
            audio: self.audio(id).map(|c| (*c).clone()),
            reverb_zone: self.reverb_zone(id).map(|c| (*c).clone()),
            canvas: self.canvas(id).map(|c| (*c).clone()),
            rect_transform: self.rect_transform(id).map(|c| (*c).clone()),
            image: self.image(id).map(|c| (*c).clone()),
            canvas_group: self.canvas_group(id).map(|c| (*c).clone()),
            rect_mask: self.rect_mask(id).map(|c| (*c).clone()),
            mask: self.mask(id).map(|c| (*c).clone()),
            backdrop_filter: self.backdrop_filter(id).map(|c| (*c).clone()),
            text: self.text(id).map(|c| (*c).clone()),
            selectable: self.selectable(id).map(|c| (*c).clone()),
            layout_group: self.layout_group(id).map(|c| (*c).clone()),
            layout_element: self.layout_element(id).map(|c| (*c).clone()),
            joint: self.joint(id).map(|c| (*c).clone()),
            character_controller: self.character_controller(id).map(|c| (*c).clone()),
            lod_group: self.lod_group(id).map(|c| (*c).clone()),
            trail: self.trail(id).map(|c| (*c).clone()),
            line: self.line(id).map(|c| (*c).clone()),
            shape: self.shape(id).map(|c| (*c).clone()),
            prefab_link: self.prefab_link(id).map(|c| (*c).clone()),
            parent_id: core.parent_id,
            parent_bone: Vec::new(), // document-only, as `bone_refs`: a save fills them
            bone_refs: BTreeMap::new(),
            children: core.children.clone(),
        })
    }

    /// Replace a live entity's whole set of columns from a document, in
    /// place — the prefab-propagation rebuild. Insertion order and the
    /// stable id keep their slots. Returns `false` when the entity does not
    /// exist.
    pub fn replace_entity(&mut self, entity: Entity) -> bool {
        self.overwrite(entity)
    }
}
