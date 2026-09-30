//! Solid contacts are collisions, not triggers (#448).
//!
//! Before #448 a static collider counted as a trigger, so a body resting on a
//! solid floor fired `OnTrigger*`. The stand-in is retired: the resting contact
//! must surface as a collision pair and never as a trigger pair.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

fn box_collider(size: Vec3, is_trigger: bool) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

#[test]
fn static_floor_contact_is_a_collision_not_a_trigger() {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    scene.world.set_static(floor, true);
    scene
        .world
        .set_collider(floor, Some(box_collider(Vec3::new(20.0, 0.5, 20.0), false)));
    let ball = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(ball).unwrap().position = Vec3::new(0.0, 1.0, 0.0);
    scene
        .world
        .set_collider(ball, Some(box_collider(Vec3::ONE, false)));
    scene.world.set_rigidbody(
        ball,
        Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            use_gravity: true,
            collision_detection: CollisionDetection::Discrete,
        }),
    );
    let want = (floor.min(ball), floor.max(ball));

    let mut physics = PhysicsWorld::from_scene(&scene);
    let mut touched = false;
    for _ in 0..240 {
        let ev = physics.step(&mut scene, 1.0 / 60.0);
        assert!(ev.triggers.is_empty(), "a solid floor is not a trigger");
        touched |= ev.collisions.stayed.iter().any(|p| p.key() == want);
    }
    assert!(
        touched,
        "a body resting on a solid floor reports a collision"
    );
}
