//! A Mesh collider whose library-mesh triangles go away in Play (#2059's review)
//! does not keep them: rebuilt from what its entity has now, or left with no
//! shape when nothing builds, so what rested on it falls.

use super::mesh_keys_tests::{plank, scene, y, DT};
use crate::components::mesh::MeshData;
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// The plank entity of `mesh_keys_tests::scene`.
fn floor(scene: &Scene) -> u32 {
    scene.mesh_users("plank")[0]
}

/// Rest the crate on the plank, take its triangles away with `take`, and return
/// the crate's height a second later and whether the plank still has a collider.
fn rest_then(always_full: bool, take: impl Fn(&mut Scene, u32)) -> (f32, bool) {
    let (mut scene, crate_) = scene();
    let floor = floor(&scene);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.tracked.always_full = always_full;
    for _ in 0..300 {
        world.step(&mut scene, DT);
    }
    assert!(
        (y(&scene, crate_) - 0.25).abs() < 0.05,
        "rests on the plank"
    );
    take(&mut scene, floor);
    for _ in 0..60 {
        world.step(&mut scene, DT);
    }
    (y(&scene, crate_), world.id_to_collider.contains_key(&floor))
}

#[test]
fn a_crate_falls_when_the_planks_mesh_is_deleted_and_comes_back_with_it() {
    for always_full in [false, true] {
        let (after, shaped) = rest_then(always_full, |s, _| {
            s.delete_mesh("plank", true).unwrap();
        });
        assert!(after < -1.0 && !shaped, "{after} ({always_full})");
    }
    let (mut scene, _) = scene();
    let floor = floor(&scene);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    scene.delete_mesh("plank", true).unwrap();
    world.step(&mut scene, DT);
    assert!(!world.id_to_collider.contains_key(&floor), "no shape");
    scene.define_mesh("plank", plank(-2.0, 2.0)).unwrap();
    scene.set_shared_mesh(floor, "plank").unwrap();
    world.step(&mut scene, DT);
    assert!(world.id_to_collider.contains_key(&floor), "built again");
}

#[test]
fn a_crate_falls_when_the_plank_draws_a_primitive_instead() {
    for always_full in [false, true] {
        // A unit Box at the plank's centre: the crate, at x = 1.5, is past it.
        let (after, shaped) = rest_then(always_full, |s, floor| {
            s.set_shared_mesh(floor, "Box").unwrap();
        });
        assert!(after < -1.0 && shaped, "{after} ({always_full})");
    }
}

#[test]
fn a_crate_falls_when_the_plank_is_replaced_with_an_empty_mesh() {
    for always_full in [false, true] {
        let (after, shaped) = rest_then(always_full, |s, _| {
            s.define_mesh("plank", MeshData::default()).unwrap();
        });
        assert!(after < -1.0 && !shaped, "{after} ({always_full})");
    }
}
