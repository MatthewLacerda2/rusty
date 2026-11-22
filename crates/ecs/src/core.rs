use rusty_components::{
    AnimatorComponent, CharacterControllerComponent, ColliderComponent, Entity, MaterialAsset,
    MeshComponent, ScriptComponent, TransformComponent,
};

use super::access::{CompMut, CompRef};
use super::bundle::Core;
use super::world::World;

impl World {
    pub fn transform(&self, id: u32) -> Option<CompRef<'_, TransformComponent>> {
        self.component::<TransformComponent>(id).map(CompRef::new)
    }

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

    pub fn is_active_self(&self, id: u32) -> bool {
        self.component::<Core>(id).is_some_and(|c| c.active)
    }

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
            self.stamp_hierarchy_below(id);
        }
        true
    }

    pub fn is_static(&self, id: u32) -> bool {
        self.component::<Core>(id).is_some_and(|c| c.is_static)
    }

    /// Not [`World::body_carrier`]'s body: a non-static Collider alone is a moving body here.
    pub fn is_moving_body(&self, id: u32) -> bool {
        !self.is_static(id) && (self.has_collider(id) || self.has_character_controller(id))
    }

    pub fn moving_bodies(&self) -> Vec<(u32, u8)> {
        let mut bodies = Vec::new();
        let mut keep = |c: &Core| {
            if !c.is_static {
                bodies.push((c.id, c.layer));
            }
        };
        self.each_core_with::<ColliderComponent>(&mut keep);
        self.each_core_with::<CharacterControllerComponent>(&mut keep);
        bodies.sort_unstable();
        bodies.dedup();
        bodies
    }

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

    /// Moves when the static set may have changed (a clear or rewind too), not on a static
    /// entity's move: that is [`World::static_geometry`]'s.
    pub fn static_revision(&self) -> u64 {
        self.static_set_revision()
    }

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

    pub fn local_and_parent(&self, id: u32) -> Option<(glam::Mat4, Option<u32>)> {
        self.with_pair::<TransformComponent, Core, _>(id, |t, c| (t.to_matrix(), c.parent_id))
    }

    pub fn parent_id(&self, id: u32) -> Option<u32> {
        self.component::<Core>(id).and_then(|c| c.parent_id)
    }

    /// At most [`World::len`] steps, so a parent cycle ends the walk; a dead parent ends it too.
    pub fn ancestors(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        let live_parent = |id: u32| self.parent_id(id).filter(|&p| self.contains(p));
        std::iter::successors(live_parent(id), move |&p| live_parent(p)).take(self.len())
    }

    pub fn self_and_ancestors(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(id).chain(self.ancestors(id))
    }

    pub fn body_carrier(&self, id: u32) -> Option<u32> {
        self.self_and_ancestors(id)
            .find(|&c| self.has_rigidbody(c) || self.has_character_controller(c))
    }

    /// ORDER: `add_child` first. A link its parent's `children` lacks makes every later activity
    /// invalidation a whole-store bump until a `clear` (`ecs::activity`).
    pub fn set_parent_id(&mut self, id: u32, parent: Option<u32>) -> bool {
        let Some(mut core) = self.component_mut::<Core>(id) else {
            return false;
        };
        let relinked = std::mem::replace(&mut core.parent_id, parent) != parent;
        drop(core);
        self.moved(id);
        if relinked {
            self.relinked_activity(id, parent);
            self.stamp_hierarchy_below(id);
        }
        true
    }

    pub fn children(&self, id: u32) -> Vec<u32> {
        self.component::<Core>(id)
            .map(|c| c.children.clone())
            .unwrap_or_default()
    }

    pub fn add_child(&mut self, parent: u32, child: u32) -> bool {
        let added = self
            .component_mut::<Core>(parent)
            .map(|mut c| c.children.push(child))
            .is_some();
        if added {
            self.stamp_children(parent);
        }
        added
    }

    pub fn remove_child(&mut self, parent: u32, child: u32) -> bool {
        let removed = self
            .component_mut::<Core>(parent)
            .map(|mut c| c.children.retain(|&c| c != child))
            .is_some();
        if removed {
            self.stamp_children(parent);
        }
        removed
    }

    pub fn take_pending_material(&mut self, id: u32) -> Option<MaterialAsset> {
        self.take_component::<MaterialAsset>(id)
    }

    pub fn stage_pending_material(&mut self, id: u32, asset: MaterialAsset) -> bool {
        self.set_component(id, Some(asset))
    }

    /// Stamps the animator alone: the palette it leads to is derived (`World::set_pose_palette`).
    pub fn with_animator_mut_and_mesh<R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut AnimatorComponent, Option<&MeshComponent>) -> R,
    ) -> Option<R> {
        self.with_mut_and_ref::<AnimatorComponent, MeshComponent, R>(id, f)
    }

    pub fn replace_entity(&mut self, entity: Entity) -> bool {
        self.overwrite(entity)
    }
}
