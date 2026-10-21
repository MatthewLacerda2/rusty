//! src/physics/shape/round_bounds_tests.rs — a collider's stored world AABB against
//! the rapier shape physics builds for it (#1140): both read one pose
//! (`World::world_pose`) and one sizing rule (`ColliderShape::scaled`), so the
//! box always holds the shape, parented and rotated or not.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rapier3d::prelude::Pose as RapierPose;

use super::build::build_shape;
use crate::components::{CapsuleAxis, ColliderComponent, ColliderShape};
use crate::physics::compound::world_pose;
use crate::scene::Scene;

const SHAPES: [ColliderShape; 4] = [
    ColliderShape::Cylinder {
        radius: 0.5,
        height: 2.0,
    },
    ColliderShape::Capsule {
        radius: 0.5,
        height: 3.0,
        axis: CapsuleAxis::X,
    },
    ColliderShape::Sphere { radius: 0.5 },
    ColliderShape::Box {
        size: Vec3::new(1.0, 2.0, 0.5),
    },
];

/// An entity with `shape`, under `parent` when given, at local `rot` and `scale`.
fn add(
    scene: &mut Scene,
    parent: Option<u32>,
    rot: Quat,
    scale: Vec3,
    shape: &ColliderShape,
) -> u32 {
    let id = scene.add_entity("E".to_string());
    if let Some(p) = parent {
        scene.set_parent(id, Some(p)).unwrap();
    }
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation, t.scale) = (Vec3::new(4.0, -1.0, 2.0), rot, scale);
    drop(t);
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = shape.clone();
    scene.world.set_collider(id, Some(c));
    scene.update_entity_collider(id);
    id
}

/// `(stored AABB, rapier shape's AABB)` for `id`.
fn bounds(scene: &Scene, id: u32) -> ((Vec3, Vec3), (Vec3, Vec3)) {
    let c = scene.world.collider(id).unwrap();
    let pose = world_pose(scene, id).unwrap();
    let want = build_shape(&c.shape, pose.scale, None)
        .unwrap()
        .shape()
        .compute_aabb(&RapierPose::from_parts(pose.pos, pose.rot));
    ((c.aabb_min, c.aabb_max), (want.mins, want.maxs))
}

/// The stored box holds the shape, and overshoots it by under `slack`.
fn assert_holds(scene: &Scene, id: u32, slack: f32) {
    let ((min, max), (wmin, wmax)) = bounds(scene, id);
    let what = &scene.world.collider(id).unwrap().shape;
    assert!(
        min.cmple(wmin + 1e-4).all() && max.cmpge(wmax - 1e-4).all(),
        "{what:?}: {min} {max} vs {wmin} {wmax}"
    );
    let over = (wmin - min).max(max - wmax).max_element();
    assert!(over < slack, "{what:?} overshoots by {over}");
}

#[test]
fn stored_bounds_match_the_shape_under_non_uniform_scale() {
    let mut scene = Scene::new();
    for shape in &SHAPES {
        let id = add(
            &mut scene,
            None,
            Quat::IDENTITY,
            Vec3::new(1.0, 2.0, 3.0),
            shape,
        );
        assert_holds(&scene, id, 1e-4);
    }
}

#[test]
fn stored_bounds_hold_a_turned_shape() {
    // A box drawn round a turned shape overshoots it: at 0.7 rad about Y, by at
    // most (cos + sin − 1) of the largest half-extent (1.5 here).
    let mut scene = Scene::new();
    for shape in &SHAPES {
        let rot = Quat::from_rotation_y(0.7);
        let id = add(&mut scene, None, rot, Vec3::new(1.0, 2.0, 3.0), shape);
        assert_holds(&scene, id, 1.5 * (0.7f32.cos() + 0.7f32.sin() - 1.0) + 1e-3);
    }
}

#[test]
fn a_turned_child_reads_its_parents_scale_as_physics_does() {
    // Parent (1,1,3), child turned 90° about Y: physics gives an X capsule radius
    // 0.5·3 = 1.5 and length 6 along world Z. Splitting the product matrix said ±0.5.
    let mut scene = Scene::new();
    let parent = scene.add_entity("P".to_string());
    scene.world.transform_mut(parent).unwrap().scale = Vec3::new(1.0, 1.0, 3.0);
    let capsule = ColliderShape::Capsule {
        radius: 0.5,
        height: 6.0,
        axis: CapsuleAxis::X,
    };
    let turned = Quat::from_rotation_y(FRAC_PI_2);
    let id = add(&mut scene, Some(parent), turned, Vec3::ONE, &capsule);
    let ((min, max), _) = bounds(&scene, id);
    let centre = Vec3::new(4.0, -1.0, 6.0);
    assert!(
        (max - centre - Vec3::new(1.5, 1.5, 3.0))
            .abs()
            .max_element()
            < 1e-4,
        "{max}"
    );
    assert!(
        (min - centre + Vec3::new(1.5, 1.5, 3.0))
            .abs()
            .max_element()
            < 1e-4,
        "{min}"
    );
    for shape in &SHAPES {
        let id = add(&mut scene, Some(parent), turned, Vec3::ONE, shape);
        assert_holds(&scene, id, 1e-3);
    }
}
