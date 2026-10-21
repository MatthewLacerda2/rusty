//! src/ecs/world_space/write.rs — the one way a Transform is written (#1170).
//!
//! [`World::transform_mut`] hands out a [`TransformMut`]: a copy of the Transform that
//! lands back in the world when the guard drops. Landing is where a write takes effect
//! in world space, once, for every writer (a script's `Transform.Set*`, a tween, the
//! editor, the physics write-back, IK, the Animator): when the value changed, the
//! entity's subtree goes dirty in the store and every collider in it gets its world
//! AABB from the new pose (#1157). An unchanged value lands as a no-op, so a guard
//! taken only to read costs nothing below it.

use std::ops::{Deref, DerefMut};

use crate::components::TransformComponent;
use crate::ecs::World;

/// A Transform being written; see the module note for what dropping it does.
pub struct TransformMut<'w> {
    world: &'w mut World,
    id: u32,
    value: TransformComponent,
}

impl Deref for TransformMut<'_> {
    type Target = TransformComponent;
    fn deref(&self) -> &TransformComponent {
        &self.value
    }
}

impl DerefMut for TransformMut<'_> {
    fn deref_mut(&mut self) -> &mut TransformComponent {
        &mut self.value
    }
}

impl Drop for TransformMut<'_> {
    fn drop(&mut self) {
        let value = std::mem::take(&mut self.value);
        self.world.land_transform(self.id, value);
    }
}

/// Whether two transforms are the same bits (a NaN never is, so it always lands).
fn same(a: &TransformComponent, b: &TransformComponent) -> bool {
    a.position == b.position && a.rotation == b.rotation && a.scale == b.scale
}

impl World {
    /// The mandatory Transform, to write. `None` only when the entity does not exist.
    pub fn transform_mut(&mut self, id: u32) -> Option<TransformMut<'_>> {
        let value = self.transform(id)?.clone();
        Some(TransformMut {
            world: self,
            id,
            value,
        })
    }

    fn land_transform(&mut self, id: u32, value: TransformComponent) {
        let Some(mut stored) = self.component_mut::<TransformComponent>(id) else {
            return;
        };
        if same(&stored, &value) {
            return;
        }
        *stored = value;
        drop(stored);
        self.moved(id);
    }

    /// `id`'s subtree moved in world space: every entity in it goes dirty, then every
    /// collider in it reads its world AABB from its new pose.
    pub(in crate::ecs) fn moved(&mut self, id: u32) {
        let subtree: Vec<u32> = self.self_and_descendants(id).collect();
        for &d in &subtree {
            self.world_space.mark_dirty(d);
        }
        for d in subtree {
            self.refresh_collider_aabb(d);
        }
    }

    /// Refresh `id`'s collider world AABB from its world pose (a no-op without a
    /// collider). The transform write does this for its whole subtree; call it after
    /// changing a collider's shape.
    pub fn refresh_collider_aabb(&mut self, id: u32) {
        if !self.has_collider(id) {
            return;
        }
        let Some(pose) = self.world_pose(id) else {
            return;
        };
        if let Some(mut col) = self.collider_mut(id) {
            let (min, max) = col.calculate_world_aabb(pose);
            col.aabb_min = min;
            col.aabb_max = max;
        }
    }
}
