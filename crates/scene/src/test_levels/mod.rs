use glam::Vec3;

use crate::create::{create_entity, Primitive};
use crate::Scene;

#[cfg(feature = "dev")]
pub mod rooms;

pub fn block(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).expect("a new entity");
    (t.position, t.scale) = ((min + max) * 0.5, max - min);
    drop(t);
    scene.world.set_static(id, true);
    id
}
