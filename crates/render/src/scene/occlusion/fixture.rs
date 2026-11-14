//! What the light-side occlusion tests share (#1714, part 4): the corridors level
//! with its PVS baked, two lamp spots in its north rooms, and a way to name one of
//! its meshes by where it stands.

use std::sync::Arc;

use glam::Vec3;

use rusty_lighting::occlusion::{bake_occlusion, test_level::corridors};
use rusty_scene::Scene;

/// In the first and the second north room, a lamp's height up.
pub(crate) const ROOM_1: Vec3 = Vec3::new(5.0, 2.0, 5.0);
pub(crate) const ROOM_2: Vec3 = Vec3::new(15.0, 2.0, 5.0);

/// The corridors level with its PVS baked.
pub(crate) fn baked() -> Scene {
    let mut scene = corridors();
    let data = bake_occlusion(&scene, 4).0.expect("the level occludes");
    *scene.occlusion = Some(Arc::new(data));
    scene
}

/// The mesh entity named `name` whose origin is nearest `at`.
pub(crate) fn nearest(scene: &Scene, name: &str, at: Vec3) -> u32 {
    let world = &scene.world;
    let d = |id: u32| world.local_to_world(id).w_axis.truncate().distance(at);
    let named = |&id: &u32| world.name(id).is_some_and(|n| n.as_str() == name);
    let ids = world.ids_with_mesh().into_iter().filter(named);
    ids.min_by(|&a, &b| d(a).total_cmp(&d(b))).expect("named")
}
