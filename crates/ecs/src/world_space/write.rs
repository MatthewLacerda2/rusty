use std::ops::{Deref, DerefMut};

use crate::World;
use rusty_components::TransformComponent;

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

fn same(a: &TransformComponent, b: &TransformComponent) -> bool {
    let bits = |t: &TransformComponent| {
        let (p, r, s) = (t.position, t.rotation, t.scale);
        [p.x, p.y, p.z, r.x, r.y, r.z, r.w, s.x, s.y, s.z].map(f32::to_bits)
    };
    bits(a) == bits(b)
}

impl World {
    pub fn transform_mut(&mut self, id: u32) -> Option<TransformMut<'_>> {
        let value = self.transform(id)?.clone();
        Some(TransformMut {
            world: self,
            id,
            value,
        })
    }

    fn land_transform(&mut self, id: u32, value: TransformComponent) {
        let Some(unchanged) = self.transform(id).map(|stored| same(&stored, &value)) else {
            return;
        };
        if unchanged {
            self.note(id);
            return;
        }
        if let Some(mut stored) = self.component_mut::<TransformComponent>(id) {
            *stored = value;
        }
        self.moved(id);
    }
}
