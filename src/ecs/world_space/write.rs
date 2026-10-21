//! src/ecs/world_space/write.rs — the one way a Transform is written (#1170).
//!
//! [`World::transform_mut`] hands out a [`TransformMut`]: a copy of the Transform that
//! lands back in the world when the guard drops. Landing is where a write takes effect
//! in world space, once, for every writer (a script's `Transform.Set*`, a tween, the
//! editor, the physics write-back, IK, the Animator): when the value changed, the
//! entity is queued as moved ([`World::moved`]), O(1): its subtree's matrices go dirty
//! before the next read and its colliders' AABBs are due a refresh (#1157). An
//! unchanged value lands as a no-op, so a guard taken only to read costs nothing.

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

/// Whether two transforms are the same bits, so a `-0.0` over a `0.0` still lands.
fn same(a: &TransformComponent, b: &TransformComponent) -> bool {
    let bits = |t: &TransformComponent| {
        let (p, r, s) = (t.position, t.rotation, t.scale);
        [p.x, p.y, p.z, r.x, r.y, r.z, r.w, s.x, s.y, s.z].map(f32::to_bits)
    };
    bits(a) == bits(b)
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
}
