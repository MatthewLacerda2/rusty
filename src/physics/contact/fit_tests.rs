//! src/physics/contact/fit_tests.rs — only a cylinder or capsule on a dynamic
//! body is built to roll, and a body that changes type in Play is refit (#2071).

use glam::{Quat, Vec3};
use rapier3d::prelude::{ActiveHooks, ShapeType};

use super::fixture::{body, DT};
use crate::components::{CapsuleAxis, ColliderShape, RigidBodyComponent};
use crate::physics::shape::fit::{build_shape_for, fit_mark, EXACT, ROLLING};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

const PILLAR: ColliderShape = ColliderShape::Cylinder {
    radius: 1.0,
    height: 2.0,
};

#[test]
fn only_a_dynamic_cylinder_or_capsule_is_built_to_roll() {
    let capsule = ColliderShape::Capsule {
        radius: 0.4,
        height: 2.0,
        axis: CapsuleAxis::Y,
    };
    let cube = ColliderShape::Box { size: Vec3::ONE };
    for (shape, rolling) in [
        (&PILLAR, ShapeType::RoundCylinder),
        (&capsule, ShapeType::Capsule),
    ] {
        for dynamic in [false, true] {
            let built = build_shape_for(1, shape, Vec3::ONE, None, dynamic).expect("built");
            let want = if dynamic { rolling } else { shape_type(shape) };
            assert_eq!(
                built.shape().shape_type(),
                want,
                "{shape:?}, dynamic {dynamic}"
            );
            let hooked = built
                .active_hooks()
                .contains(ActiveHooks::MODIFY_SOLVER_CONTACTS);
            assert_eq!(hooked, dynamic, "{shape:?} kept out of recycling");
            let stamp = if dynamic { ROLLING } else { EXACT };
            assert_eq!(built.user_data, stamp, "{shape:?} stamp");
            assert_eq!(fit_mark(shape, dynamic), stamp);
        }
    }
    let crate_box = build_shape_for(1, &cube, Vec3::ONE, None, true).expect("built");
    assert!(
        crate_box.active_hooks().is_empty(),
        "a box is recycled as before"
    );
}

fn shape_type(shape: &ColliderShape) -> ShapeType {
    match shape {
        ColliderShape::Cylinder { .. } => ShapeType::Cylinder,
        _ => ShapeType::Capsule,
    }
}

#[test]
fn a_body_turned_kinematic_in_play_gets_its_exact_cylinder_back() {
    let mut scene = Scene::new();
    let still = Vec3::ZERO;
    let id = body(&mut scene, PILLAR, Quat::IDENTITY, Vec3::ZERO, still, still);
    scene.world.rigidbody_mut(id).unwrap().use_gravity = false;
    let mut world = PhysicsWorld::from_scene(&scene);
    let shape = |world: &PhysicsWorld| {
        world.colliders[world.id_to_collider[&id]]
            .shape()
            .shape_type()
    };
    assert_eq!(shape(&world), ShapeType::RoundCylinder);
    let set = |scene: &mut Scene, kinematic| {
        let rb: &mut RigidBodyComponent = &mut scene.world.rigidbody_mut(id).unwrap();
        rb.is_kinematic = kinematic;
    };
    set(&mut scene, true);
    world.step(&mut scene, DT);
    assert_eq!(shape(&world), ShapeType::Cylinder);
    let hooks = world.colliders[world.id_to_collider[&id]].active_hooks();
    assert!(hooks.is_empty(), "a kinematic cylinder is recycled again");
    set(&mut scene, false);
    world.step(&mut scene, DT);
    assert_eq!(shape(&world), ShapeType::RoundCylinder);
}
