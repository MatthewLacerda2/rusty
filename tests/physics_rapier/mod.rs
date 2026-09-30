//! rapier3d-backed physics: gravity, kinematic drive, static blocking, raycast.

mod bodies;
mod queries;

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
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

fn dynamic_body() -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: true,
        collision_detection: CollisionDetection::Discrete,
    }
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}
