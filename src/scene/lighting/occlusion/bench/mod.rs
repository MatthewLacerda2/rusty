//! The primitive levels the occlusion bake is measured on: rooms on two storeys
//! (`level`, Dust2-sized at its largest) and a town of buildings and courtyards
//! (`town`, #1981), plus [`shift`], which moves a level off round coordinates.

pub mod level;
pub mod town;

use glam::Vec3;

use crate::scene::Scene;

/// Move and scale every mesh of `scene` as one: a point `p` goes to `origin +
/// p * scale`. A level built on round coordinates becomes one whose corners,
/// as an imported level's do, fill their floats' low bits.
pub fn shift(scene: &mut Scene, origin: Vec3, scale: f32) {
    for id in scene.world.ids_with_mesh() {
        let mut t = scene.world.transform_mut(id).expect("a mesh's entity");
        (t.position, t.scale) = (origin + t.position * scale, t.scale * scale);
    }
}
