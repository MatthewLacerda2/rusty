//! crates/ecs/src/core.rs — the identity-core facade + mandatory components (#344, #345).
//!
//! Access to the facts every GameObject has — the slim "core component" of
//! #343's target model (`active`, `is_static`, `layer`, hierarchy links) —
//! plus the mandatory `name`, `Transform`, and scripts list, each its own
//! hecs column (#345), and `replace_entity`, the whole-bundle write prefab
//! propagation needs (assembling a document is `bundle.rs`'s).
//!
//! An owned `Entity` VALUE is a *document* (the serialized scene shape, kept
//! as the on-disk format): constructing one, serializing one, or diffing two
//! is fine anywhere. What consumers must not do is reach into live storage
//! directly — that lives in `world.rs`'s generic column accessors, used only
//! from this module and `access.rs`.

use rusty_components::{
    AnimatorComponent, Entity, MaterialAsset, MeshComponent, ScriptComponent, TransformComponent,
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
            self.invalidate_activity_below(id);
            // The subtree may go dark or lit; with no static in it (a muzzle flash, a
            // pickup) the static bakes stay put.
            self.stamp_hierarchy_below(id);
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
            self.stamp_static(id);
        }
        true
    }

    /// The [`StaticSet`](super::changes::StaticSet) channel's revision: it moves
    /// whenever the set of static entities may have changed, the World cleared or
    /// rewound included, so a cache of static geometry keyed on it (the renderer's
    /// static shadow bakes) goes stale (#1496). A cache that wants *which* entities
    /// moved opens a cursor on that channel.
    pub fn static_revision(&self) -> u64 {
        self.static_set_revision()
    }

    /// The entity's layer index (`0`, the default layer, for a dead id).
    pub fn layer(&self, id: u32) -> u8 {
        self.component::<Core>(id).map(|c| c.layer).unwrap_or(0)
    }

    pub fn set_layer(&mut self, id: u32, layer: u8) -> bool {
        let Some(mut core) = self.component_mut::<Core>(id) else {
            return false;
        };
        let changed = std::mem::replace(&mut core.layer, layer) != layer;
        drop(core);
        changed.then(|| self.stamp_layer(id));
        true
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

    /// Write the raw parent link; cycle checks and `children` are `Scene::set_parent`'s.
    /// ORDER: `add_child` first. A link to a parent whose `children` lacks `id` marks
    /// the activity cache `unlisted`, every later invalidation a whole-store bump
    /// until a `clear` (`ecs::activity`). The subtree moves in world space, so its
    /// world matrices go dirty and its colliders' AABBs follow (`ecs::world_space`).
    pub fn set_parent_id(&mut self, id: u32, parent: Option<u32>) -> bool {
        let Some(mut core) = self.component_mut::<Core>(id) else {
            return false;
        };
        let relinked = std::mem::replace(&mut core.parent_id, parent) != parent;
        drop(core);
        self.moved(id);
        if relinked {
            self.relinked_activity(id, parent);
            self.stamp_hierarchy_below(id); // under a new parent it may go dark or lit
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

    /// The one sanctioned split borrow: mutate an entity's animator while reading
    /// its mesh (when present) — the `animate` system's re-pose step. Only the
    /// animator is stamped: the step reads the clips and skeleton, and the palette
    /// it leads to is derived (`World::set_pose_palette`), so animated meshes do not
    /// grow the despawn index's work (#1708). `None` when missing or animator-less.
    pub fn with_animator_mut_and_mesh<R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut AnimatorComponent, Option<&MeshComponent>) -> R,
    ) -> Option<R> {
        self.with_mut_and_ref::<AnimatorComponent, MeshComponent, R>(id, f)
    }

    /// Replace a live entity's whole set of columns from a document, in
    /// place — the prefab-propagation rebuild. Insertion order and the
    /// stable id keep their slots. Returns `false` when the entity does not
    /// exist.
    pub fn replace_entity(&mut self, entity: Entity) -> bool {
        self.overwrite(entity)
    }
}
