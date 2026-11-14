//! Root motion on a physics body (#1745): a dynamic Rigidbody takes the lunge
//! as velocity (the solver owns its pose) and a wall stops it, and a
//! CharacterController carries a creep shorter than its `min_move_distance`
//! instead of dropping it.

use glam::Vec3;

use super::tests::{add_box, lunging, play, playing};
use rusty_components::{
    CharacterControllerComponent, ColliderComponent, ColliderShape, RigidBodyComponent,
};
use rusty_scene::Scene;

/// Give `id` a dynamic, weightless, frictionless box body.
fn dynamic(scene: &mut Scene, id: u32) {
    let rb = RigidBodyComponent {
        is_kinematic: false,
        use_gravity: false,
        ..rusty_scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    let mut collider = ColliderComponent {
        shape: ColliderShape::Box {
            size: Vec3::splat(0.5),
        },
        ..rusty_scene::authoring::default_collider()
    };
    collider.material.friction = 0.0;
    scene.world.set_collider(id, Some(collider));
}

#[test]
fn a_dynamic_body_lunges_two_metres_on_its_velocity() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    dynamic(&mut scene, guard);
    let trace = play(scene, guard);
    let (end, hips) = trace[89];
    assert!((end.x - 2.0).abs() < 0.05, "moved 2 m: {end}");
    assert!(
        end.y.abs() < 1e-3 && end.z.abs() < 1e-3,
        "only along X: {end}"
    );
    assert!((hips.x - end.x).abs() < 1e-3, "the hips stay with the body");
    let mid = trace[30].0.x - trace[29].0.x;
    assert!((mid - 2.0 / 60.0).abs() < 1e-3, "a steady 2 m/s: {mid}");
}

#[test]
fn a_wall_stops_a_dynamic_bodys_lunge() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    dynamic(&mut scene, guard);
    add_box(&mut scene, Vec3::X * 1.5, Vec3::new(0.2, 4.0, 4.0));
    let (end, _) = play(scene, guard)[89];
    assert!(end.x > 0.9 && end.x < 1.4, "stopped at the wall: {end}");
}

#[test]
fn a_character_controller_carries_a_creep_below_its_min_move() {
    let mut scene = Scene::new();
    // 0.2 m over a second: 3.3 mm a step, under a 1 cm minimum move.
    let guard = playing(&mut scene, 0.2);
    let cc = CharacterControllerComponent {
        min_move_distance: 0.01,
        ..CharacterControllerComponent::default()
    };
    scene.world.set_character_controller(guard, Some(cc));
    let (end, _) = play(scene, guard)[89];
    assert!((end.x - 0.2).abs() < 0.011, "the creep adds up: {end}");
}
