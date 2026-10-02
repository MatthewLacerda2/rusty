//! src/physics/live_tests.rs — mid-play edits reach the built bodies (#466): a
//! Rigidbody's mass, a kinematic switch and a layer change, plus the velocity
//! change an impulse at a point gives.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A body at `pos` with a `size` box collider: dynamic unless `is_static`.
fn body(scene: &mut Scene, pos: Vec3, size: Vec3, is_static: bool) -> u32 {
    let id = scene.add_entity("Body".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(id, Some(collider));
    if is_static {
        scene.world.set_static(id, true);
    } else {
        let rb = RigidBodyComponent {
            is_kinematic: false,
            ..crate::scene::authoring::defaults::default_rigidbody()
        };
        scene.world.set_rigidbody(id, Some(rb));
    }
    id
}

fn y(scene: &Scene, id: u32) -> f32 {
    scene.world.transform(id).unwrap().position.y
}

#[test]
fn a_rigidbody_mass_is_the_body_mass_whatever_its_size() {
    let mut scene = Scene::new();
    let small = body(&mut scene, Vec3::ZERO, Vec3::splat(0.1), false);
    scene.world.rigidbody_mut(small).unwrap().mass = 5.0;
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    let handle = world.id_to_body[&small];
    assert!((world.bodies[handle].mass() - 5.0).abs() < 1e-4);
    // An impulse of 10 N·s through the centre adds 2 m/s and no spin.
    let centre = scene.world.transform(small).unwrap().position;
    let (dv, dw) = world
        .impulse_response(small, Vec3::X * 10.0, centre)
        .unwrap();
    assert!(dv.abs_diff_eq(Vec3::X * 2.0, 1e-4), "{dv}");
    assert!(dw.length() < 1e-4, "{dw}");
    // Pushed +X above the centre, it spins about -Z (top forward).
    let (_, dw) = world
        .impulse_response(small, Vec3::X, centre + Vec3::Y * 0.05)
        .unwrap();
    assert!(dw.z < 0.0 && dw.x.abs() < 1e-4 && dw.y.abs() < 1e-4, "{dw}");
}

#[test]
fn set_kinematic_mid_play_stops_and_restarts_a_falling_body() {
    let mut scene = Scene::new();
    let id = body(&mut scene, Vec3::new(0.0, 10.0, 0.0), Vec3::ONE, false);
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..10 {
        world.step(&mut scene, DT);
    }
    let held = y(&scene, id);
    assert!(held < 10.0, "it falls while dynamic");
    scene.world.rigidbody_mut(id).unwrap().is_kinematic = true;
    for _ in 0..10 {
        world.step(&mut scene, DT);
    }
    assert!((y(&scene, id) - held).abs() < 1e-5, "kinematic: it holds");
    scene.world.rigidbody_mut(id).unwrap().is_kinematic = false;
    for _ in 0..10 {
        world.step(&mut scene, DT);
    }
    assert!(y(&scene, id) < held - 0.1, "dynamic again: it falls");
}

#[test]
fn a_layer_change_mid_play_decides_what_the_body_lands_on() {
    let fall = |switch_back: bool| {
        let mut scene = Scene::new();
        body(
            &mut scene,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(10.0, 1.0, 10.0),
            true,
        );
        let id = body(&mut scene, Vec3::new(0.0, 1.0, 0.0), Vec3::ONE, false);
        scene.collision_matrix.set_collision(5, 0, false);
        scene.world.set_layer(id, 5);
        let mut world = PhysicsWorld::from_scene(&scene);
        for tick in 0..90 {
            if tick == 5 && switch_back {
                scene.world.set_layer(id, 0);
            }
            world.step(&mut scene, DT);
        }
        y(&scene, id)
    };
    assert!(
        fall(false) < -1.0,
        "a layer that ignores the floor falls through"
    );
    assert!(
        (fall(true) - 0.5).abs() < 0.05,
        "moved onto Default, it lands"
    );
}
