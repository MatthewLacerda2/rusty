//! A Mesh collider built from a library mesh follows a replace in Play (#2059's
//! review): what rests on a plank that is halved falls, through the incremental
//! sync and the full pass alike.

use glam::Vec3;

use crate::components::mesh::MeshData;
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

pub(super) const DT: f32 = 1.0 / 60.0;

/// A flat quad at y = 0 from x = `x0` to `x1`, z from -2 to 2, facing up.
pub(super) fn plank(x0: f32, x1: f32) -> MeshData {
    MeshData {
        positions: vec![
            [x0, 0.0, 2.0],
            [x1, 0.0, 2.0],
            [x1, 0.0, -2.0],
            [x0, 0.0, -2.0],
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    }
}

/// A static plank drawing the library mesh "plank" with a Mesh collider, and a
/// dynamic half-metre box above its right half.
pub(super) fn scene() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.define_mesh("plank", plank(-2.0, 2.0)).unwrap();
    let floor = scene.add_entity("Plank".into());
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    scene.world.set_collider(floor, Some(collider));
    scene.set_shared_mesh(floor, "plank").unwrap();
    scene.world.set_static(floor, true);
    let crate_ = scene.add_entity("Crate".into());
    scene.world.transform_mut(crate_).unwrap().position = Vec3::new(1.5, 0.3, 0.0);
    let shape = ColliderShape::Box {
        size: Vec3::splat(0.5),
    };
    let collider = ColliderComponent {
        shape,
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(crate_, Some(collider));
    let rb = crate::scene::authoring::defaults::default_rigidbody();
    scene.world.set_rigidbody(
        crate_,
        Some(RigidBodyComponent {
            is_kinematic: false,
            ..rb
        }),
    );
    (scene, crate_)
}

pub(super) fn y(scene: &Scene, id: u32) -> f32 {
    scene.world.transform(id).unwrap().position.y
}

/// The crate's height at rest on the whole plank, and a second after the plank
/// is replaced with its left half.
fn rest_then_halve(always_full: bool) -> (f32, f32) {
    let (mut scene, crate_) = scene();
    let mut world = PhysicsWorld::from_scene(&scene);
    world.tracked.always_full = always_full;
    for _ in 0..300 {
        world.step(&mut scene, DT);
    }
    let rest = y(&scene, crate_);
    assert!(
        world.bodies[world.id_to_body[&crate_]].is_sleeping(),
        "asleep at rest"
    );
    // The same data again re-shapes nothing and keeps it resting.
    scene.define_mesh("plank", plank(-2.0, 2.0)).unwrap();
    world.step(&mut scene, DT);
    assert!((y(&scene, crate_) - rest).abs() < 1e-3);
    scene.define_mesh("plank", plank(-2.0, 0.0)).unwrap();
    for _ in 0..60 {
        world.step(&mut scene, DT);
    }
    (rest, y(&scene, crate_))
}

#[test]
fn a_crate_on_a_halved_plank_falls() {
    for always_full in [false, true] {
        let (rest, after) = rest_then_halve(always_full);
        assert!((rest - 0.25).abs() < 0.05, "rests on the plank: {rest}");
        assert!(
            after < -1.0,
            "falls once the plank is halved: {after} ({always_full})"
        );
    }
}
