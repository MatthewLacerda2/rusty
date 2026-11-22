//! Test hooks: the bench levels other crates' cost and GPU tests bake.

pub use rusty_scene::test_levels::rooms as level;
pub mod town;

use glam::Vec3;

use rusty_scene::Scene;

pub fn shift(scene: &mut Scene, origin: Vec3, scale: f32) {
    for id in scene.world.ids_with_mesh() {
        let mut t = scene.world.transform_mut(id).expect("a mesh's entity");
        (t.position, t.scale) = (origin + t.position * scale, t.scale * scale);
    }
}
