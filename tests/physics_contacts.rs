//! Contact-pair reporting: a *solid* static body still surfaces its contacts.
//!
//! `collect_triggers` reports a pair when an active contact involves a trigger or a
//! static body, and only one side needs to qualify. This guards that contract: a
//! dynamic body resting on a non-trigger static floor must be reported.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

fn box_collider(size: Vec3, is_trigger: bool) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

#[test]
fn static_floor_contact_is_reported() {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    {
        let mut e = scene.get_entity_mut(floor).unwrap();
        e.is_static = true;
        e.collider = Some(box_collider(Vec3::new(20.0, 0.5, 20.0), false));
    }
    let ball = scene.add_entity("Ball".to_string());
    {
        let mut e = scene.get_entity_mut(ball).unwrap();
        e.transform.position = Vec3::new(0.0, 1.0, 0.0);
        e.collider = Some(box_collider(Vec3::ONE, false));
        e.rigidbody = Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            use_gravity: true,
            collision_detection: CollisionDetection::Discrete,
        });
    }
    // collect_triggers orders each pair (low, high).
    let want = (floor.min(ball), floor.max(ball));

    let mut physics = PhysicsWorld::from_scene(&scene);
    let mut saw = false;
    for _ in 0..240 {
        if physics.step(&mut scene, 1.0 / 60.0).stayed.contains(&want) {
            saw = true;
            break;
        }
    }
    assert!(
        saw,
        "a body resting on a solid static floor should report the contact pair"
    );
}
