//! glam splits a zero axis into a NaN rotation; the pose physics and the stored AABB share must stay finite.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rapier3d::prelude::Pose as RapierPose;

use super::compound::world_pose;
use super::shape::build::build_shape;
use super::PhysicsWorld;
use rusty_components::{CapsuleAxis, ColliderComponent, ColliderShape};
use rusty_scene::Scene;

fn child_under(scene: &mut Scene, parent_scale: Vec3) -> u32 {
    let parent = scene.add_entity("P".to_string());
    let mut t = scene.world.transform_mut(parent).unwrap();
    (t.position, t.rotation, t.scale) = (
        Vec3::new(1.0, 2.0, 3.0),
        Quat::from_rotation_y(0.6),
        parent_scale,
    );
    drop(t);
    let id = scene.add_entity("C".to_string());
    scene.set_parent(id, Some(parent)).unwrap();
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation) = (Vec3::X, Quat::from_rotation_z(FRAC_PI_2));
    drop(t);
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = ColliderShape::Capsule {
        radius: 0.5,
        height: 2.0,
        axis: CapsuleAxis::Y,
    };
    scene.world.set_collider(id, Some(c));
    id
}

const DEGENERATE: [Vec3; 2] = [Vec3::new(0.0, 1.0, 1.0), Vec3::ZERO];

#[test]
fn a_zero_scaled_parent_gives_a_finite_pose_and_aabb() {
    for parent_scale in DEGENERATE {
        let mut scene = Scene::new();
        let id = child_under(&mut scene, parent_scale);
        let pose = world_pose(&scene, id).unwrap();
        assert!(pose.rot.is_finite(), "{parent_scale}: {:?}", pose.rot);
        assert!(pose.pos.is_finite() && pose.scale.is_finite());
        let c = scene.world.collider(id).unwrap();
        assert!(c.aabb_min.is_finite() && c.aabb_max.is_finite(), "{:?}", *c);
        let rapier = build_shape(&c.shape, pose.scale, None)
            .unwrap()
            .shape()
            .compute_aabb(&RapierPose::from_parts(pose.pos, pose.rot));
        assert!(rapier.mins.is_finite() && rapier.maxs.is_finite());
        // Building the physics world from it must not panic on a NaN pose.
        PhysicsWorld::from_scene(&scene);
    }
}
