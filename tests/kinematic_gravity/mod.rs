//! Kinematic-body gravity (#318): a `use_gravity` kinematic character falls,
//! lands on static ground, rests without jitter, and starts a *fresh* fall when
//! stepping off a ledge (grounded contact zeroes the accumulated fall speed).

mod falling;
mod grounding;

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn box_collider(size: Vec3) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn kinematic_body(use_gravity: bool) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity,
        collision_detection: CollisionDetection::Discrete,
    }
}

/// Static floor slab: top face at y = 0.5.
fn add_floor(scene: &mut Scene, size: Vec3) -> u32 {
    let id = scene.add_entity("Floor".to_string());
    scene.world.set_static(id, true);
    scene.world.set_collider(id, Some(box_collider(size)));
    id
}

/// Unit-box character at `pos`, kinematic, with the given gravity opt-in.
fn add_character(scene: &mut Scene, pos: Vec3, rb: Option<RigidBodyComponent>) -> u32 {
    let id = scene.add_entity("Character".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_collider(id, Some(box_collider(Vec3::ONE)));
    scene.world.set_rigidbody(id, rb);
    id
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}
