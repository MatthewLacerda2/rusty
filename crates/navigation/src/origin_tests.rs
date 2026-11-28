use glam::{IVec3, Vec3};

use crate::NavigationGraph;
use rusty_scene::Scene;

fn floor_scene() -> Scene {
    let mut s = Scene::new();
    let id = rusty_scene::create::create_entity(
        &mut s,
        "Floor",
        Some(rusty_scene::create::Primitive::Plane),
    );
    if let Some(mut t) = s.world.transform_mut(id) {
        t.position = Vec3::new(30_000.0, 0.0, 0.0);
        t.scale = Vec3::splat(4.0);
    }
    s.world.set_static(id, true);
    s
}

#[test]
fn a_shift_moves_the_navmesh_with_the_floor_and_rebakes_nothing() {
    let mut scene = floor_scene();
    let mut nav = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 0.25);
    nav.start_tracked(&mut scene);
    let generation = nav.bake_generation;
    let on = nav.world_to_grid(Vec3::new(30_000.0, 0.0, 0.0));
    nav.sync_changes(&mut scene);
    let by = scene.shift_origin(IVec3::new(29, 0, 0));
    nav.shift_origin(&mut scene, by);
    assert_eq!(nav.world_to_grid(Vec3::new(30_000.0, 0.0, 0.0) + by), on);
    assert_eq!(nav.sync_changes(&mut scene), crate::Rebake::Unchanged);
    assert_eq!(nav.bake_generation, generation, "no bake after the shift");
}

#[test]
fn a_path_asked_after_the_shift_is_the_one_before_moved_by_it() {
    let mut scene = floor_scene();
    let mut nav = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 0.25);
    nav.start_tracked(&mut scene);
    let (a, b) = (
        Vec3::new(29_997.0, 0.0, -2.0),
        Vec3::new(30_003.0, 0.0, 2.0),
    );
    let before = nav.calculate_path(a, b).corners;
    nav.sync_changes(&mut scene);
    let by = scene.shift_origin(IVec3::new(29, 0, 0));
    nav.shift_origin(&mut scene, by);
    let after = nav.calculate_path(a + by, b + by).corners;
    assert!(
        !before.is_empty() && before.len() == after.len(),
        "{before:?} {after:?}"
    );
    for (p, q) in before.iter().zip(&after) {
        assert!((*p + by - *q).length() < 0.01, "{p} moved to {q}");
    }
}
