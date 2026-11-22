use std::ops::{Deref, DerefMut};

use crate::{CompRef, World};
use rusty_components::ColliderComponent;

/// A collider being written: on drop it lands the edit (nothing when unchanged) and refreshes the
/// AABB only when the shape changed.
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
        let Some(value) = self.value.take() else {
            return;
        };
        let Some(stored) = self
            .world
            .collider(self.id)
            .map(|c| ColliderComponent::clone(&c))
        else {
            return;
        };
        if stored == value {
            return;
        }
        let reshaped = stored.shape != value.shape;
        self.world.set_component(self.id, Some(value));
        if reshaped {
            self.world.refresh_collider_aabb(self.id);
        }
    }
}

impl World {
    pub fn collider(&self, id: u32) -> Option<CompRef<'_, ColliderComponent>> {
        self.component::<ColliderComponent>(id).map(CompRef::new)
    }

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

    /// Refreshes the AABB on attach, so a collider added in Play never keeps a zero box.
    pub fn set_collider(&mut self, id: u32, value: Option<ColliderComponent>) -> bool {
        let attached = value.is_some();
        let set = self.set_component(id, value);
        if set && attached {
            self.refresh_collider_aabb(id);
        }
        set
    }

    pub fn ids_with_collider(&self) -> Vec<u32> {
        self.ids_with_component::<ColliderComponent>()
    }
}
