//! rapier3d-backed physics: gravity, kinematic drive, static blocking, raycast.

mod bodies;
mod queries;

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty::scene::Scene;

fn box_collider(size: Vec3, is_trigger: bool) -> ColliderComponent {
    ColliderComponent {
        shape: ColliderShape::Box { size },
        is_trigger,
        ..rusty::scene::authoring::default_collider()
    }
}

fn dynamic_body() -> RigidBodyComponent {
    rusty::scene::authoring::default_rigidbody()
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}
