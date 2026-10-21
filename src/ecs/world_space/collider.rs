//! src/ecs/world_space/collider.rs — the Collider's accessors, which keep its AABB set.
//!
//! The other optional components get theirs from `access.rs`'s macro. A collider's
//! world AABB depends on its shape, so the two ways to change one refresh it: attaching
//! it ([`World::set_collider`]) and writing it through [`ColliderMut`], which lands the
//! edit and refreshes the box when the guard drops, like `TransformMut`. Adding a
//! collider in Play or reshaping it in the editor therefore never leaves a zero box.

use std::ops::{Deref, DerefMut};

use crate::components::ColliderComponent;
use crate::ecs::{CompRef, World};

/// A collider being written; dropping it lands the edit and refreshes the AABB.
pub struct ColliderMut<'w> {
    world: &'w mut World,
    id: u32,
    value: Option<ColliderComponent>,
}

impl Deref for ColliderMut<'_> {
    type Target = ColliderComponent;
    fn deref(&self) -> &ColliderComponent {
        self.value.as_ref().expect("taken only on drop")
    }
}

impl DerefMut for ColliderMut<'_> {
    fn deref_mut(&mut self) -> &mut ColliderComponent {
        self.value.as_mut().expect("taken only on drop")
    }
}

impl Drop for ColliderMut<'_> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.world.set_collider(self.id, Some(value));
        }
    }
}

impl World {
    pub fn collider(&self, id: u32) -> Option<CompRef<'_, ColliderComponent>> {
        self.component::<ColliderComponent>(id).map(CompRef::new)
    }

    /// The collider, to write; `None` when absent.
    pub fn collider_mut(&mut self, id: u32) -> Option<ColliderMut<'_>> {
        let value = Some(self.collider(id)?.clone());
        Some(ColliderMut {
            world: self,
            id,
            value,
        })
    }

    pub fn has_collider(&self, id: u32) -> bool {
        self.has_component::<ColliderComponent>(id)
    }

    /// Attach (`Some`, its AABB refreshed from the entity's world pose) or detach
    /// (`None`) the collider. Returns `false` when the entity does not exist.
    pub fn set_collider(&mut self, id: u32, value: Option<ColliderComponent>) -> bool {
        let attached = value.is_some();
        let set = self.set_component(id, value);
        if set && attached {
            self.refresh_collider_aabb(id);
        }
        set
    }

    /// Detach and return the collider (`None` when absent or the entity is dead).
    pub fn take_collider(&mut self, id: u32) -> Option<ColliderComponent> {
        self.take_component::<ColliderComponent>(id)
    }

    /// Stable ids of the entities carrying a collider, in insertion order.
    pub fn ids_with_collider(&self) -> Vec<u32> {
        self.ids_with_component::<ColliderComponent>()
    }
}
