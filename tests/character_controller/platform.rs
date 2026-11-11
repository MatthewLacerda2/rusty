//! A moving kinematic body under a walking controller carries it nowhere, as
//! in Unity: a game that wants a lift to carry its player adds the lift's
//! motion to the `Move` itself. rapier's controller rode such a body, but only
//! on the slides of a move that touched it, and its ride is what stalled a
//! walk on a resting one (#2061).

use glam::{Quat, Vec3};
use rusty::physics::{self, PhysicsWorld};
use rusty::scene::authoring::default_rigidbody;
use rusty::scene::Scene;

use super::{add_box, add_character, pos_of, DT};

#[test]
fn a_moving_kinematic_floor_does_not_carry_the_walker() {
    let mut scene = Scene::new();
    let size = Vec3::new(20.0, 1.0, 20.0);
    let deck = add_box(&mut scene, Vec3::new(0.0, -0.5, 0.0), size, Quat::IDENTITY);
    scene.world.set_static(deck, false);
    let body = rusty::components::RigidBodyComponent {
        is_kinematic: true,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(deck, Some(body));
    let id = add_character(&mut scene, Vec3::ZERO);
    let mut physics = PhysicsWorld::from_scene(&scene);
    let ticks = 60;
    for _ in 0..ticks {
        // The deck slides +x at 3 m/s; the walker heads +z at 4 m/s.
        scene.world.transform_mut(deck).unwrap().position.x += 3.0 * DT;
        let motion = Vec3::new(0.0, -0.02, 4.0 * DT);
        physics::move_character(Some(&physics), &mut scene, id, motion).unwrap();
        physics.step(&mut scene, DT);
    }
    let p = pos_of(&scene, id);
    assert!(p.x.abs() < 1e-2, "carried sideways to x={}", p.x);
    let wanted = 4.0 * DT * ticks as f32;
    assert!(p.z > wanted * 0.99, "walked {} of {wanted}", p.z);
}
