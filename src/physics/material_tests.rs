//! src/physics/material_tests.rs — capsule and physics-material behaviour (#447).
//!
//! Live-simulation checks: a capsule rests at its full-height semantics, a
//! bouncy sphere rebounds to the height its restitution predicts, and the
//! combine-mode table is read back as the exact coefficients rapier's solver
//! uses on the contact.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{
    CapsuleAxis, ColliderComponent, ColliderShape, CombineMode, PhysicsMaterial, RigidBodyComponent,
};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn collider(shape: ColliderShape, material: PhysicsMaterial) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape,
        is_trigger: false,
        material,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

/// A scene with a static floor whose top face is `y = 0`, and a dynamic body
/// with `shape` at `pos`. Returns `(scene, body id)`.
fn floor_and_body(
    shape: ColliderShape,
    pos: Vec3,
    floor_mat: PhysicsMaterial,
    body_mat: PhysicsMaterial,
) -> (Scene, u32) {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    scene.world.transform_mut(floor).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
    scene.world.set_static(floor, true);
    let size = Vec3::new(20.0, 1.0, 20.0);
    let floor_col = collider(ColliderShape::Box { size }, floor_mat);
    scene.world.set_collider(floor, Some(floor_col));

    let body = scene.add_entity("Body".to_string());
    scene.world.transform_mut(body).unwrap().position = pos;
    scene
        .world
        .set_collider(body, Some(collider(shape, body_mat)));
    scene.world.set_rigidbody(
        body,
        Some(RigidBodyComponent {
            is_kinematic: false,
            ..crate::scene::authoring::defaults::default_rigidbody()
        }),
    );
    (scene, body)
}

fn y_after(scene: &mut Scene, world: &mut PhysicsWorld, id: u32, ticks: usize) -> f32 {
    for _ in 0..ticks {
        world.step(scene, DT);
    }
    scene.world.transform(id).unwrap().position.y
}

#[test]
fn capsule_rests_on_its_full_height() {
    let d = PhysicsMaterial::default();
    for (axis, rest_y) in [(CapsuleAxis::Y, 1.0), (CapsuleAxis::X, 0.5)] {
        let shape = ColliderShape::Capsule {
            radius: 0.5,
            height: 2.0,
            axis,
        };
        let (mut scene, id) = floor_and_body(shape, Vec3::new(0.0, 3.0, 0.0), d, d);
        let mut world = PhysicsWorld::from_scene(&scene);
        let y = y_after(&mut scene, &mut world, id, 240);
        assert!(
            (y - rest_y).abs() < 0.03,
            "{axis:?} capsule rests at {y}, want {rest_y}"
        );
    }
}

/// Drop a unit-diameter sphere from 2 m (bottom to floor) with both colliders
/// at `bounciness`, and return the peak height its bottom reaches after the
/// first bounce.
fn rebound_height(bounciness: f32) -> f32 {
    let m = PhysicsMaterial::new(0.5, bounciness);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    let (mut scene, id) = floor_and_body(shape, Vec3::new(0.0, 2.5, 0.0), m, m);
    let mut world = PhysicsWorld::from_scene(&scene);
    let mut bounced = false;
    let mut peak = 0.0_f32;
    for _ in 0..240 {
        world.step(&mut scene, DT);
        let vy = scene.world.rigidbody(id).unwrap().velocity.y;
        let bottom = scene.world.transform(id).unwrap().position.y - 0.5;
        bounced |= vy > 0.0 && bottom < 0.5;
        if bounced {
            if vy <= 0.0 {
                break;
            }
            peak = peak.max(bottom);
        }
    }
    peak
}

#[test]
fn bouncy_sphere_rebounds_to_restitution_squared_times_drop() {
    // Ballistic: rebound height = e² · drop height (2 m).
    for e in [0.8_f32, 0.5] {
        let want = e * e * 2.0;
        let got = rebound_height(e);
        assert!(
            (got - want).abs() < 0.15 * want,
            "e = {e}: rebound {got}, want ≈{want}"
        );
    }
    assert!(
        rebound_height(0.0) < 0.05,
        "a dead material must not bounce"
    );
}

/// Rest a sphere on the floor and read the (friction, restitution) rapier's
/// solver combined for their contact.
fn contact_coefficients(floor: PhysicsMaterial, ball: PhysicsMaterial) -> (f32, f32) {
    let shape = ColliderShape::Sphere { radius: 0.5 };
    let (mut scene, _) = floor_and_body(shape, Vec3::new(0.0, 0.5, 0.0), floor, ball);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    let contact = world
        .narrow_phase
        .contact_pairs()
        .flat_map(|p| p.manifolds.iter())
        .flat_map(|m| m.data.solver_contacts.iter())
        .next()
        .expect("the resting sphere touches the floor");
    (contact.friction, contact.restitution)
}

fn material(friction: f32, bounce: f32, fc: CombineMode, bc: CombineMode) -> PhysicsMaterial {
    PhysicsMaterial {
        friction_combine: fc,
        bounce_combine: bc,
        ..PhysicsMaterial::new(friction, bounce)
    }
}

#[test]
fn combine_modes_resolve_by_priority_to_exact_coefficients() {
    use CombineMode::*;
    // (floor modes, ball modes, want friction of 0.8 & 0.2, want bounce of 0.6 & 0.3)
    let table = [
        ((Average, Average), (Average, Average), 0.5, 0.45),
        ((Average, Minimum), (Minimum, Average), 0.2, 0.3),
        ((Minimum, Multiply), (Multiply, Minimum), 0.16, 0.18),
        ((Multiply, Maximum), (Maximum, Average), 0.8, 0.6),
        ((Maximum, Average), (Average, Multiply), 0.8, 0.18),
    ];
    for ((ff, fb), (bf, bb), want_f, want_b) in table {
        let floor = material(0.8, 0.6, ff, fb);
        let ball = material(0.2, 0.3, bf, bb);
        let (f, b) = contact_coefficients(floor, ball);
        assert!(
            (f - want_f).abs() < 1e-5,
            "friction {ff:?}/{bf:?}: {f}, want {want_f}"
        );
        assert!(
            (b - want_b).abs() < 1e-5,
            "bounce {fb:?}/{bb:?}: {b}, want {want_b}"
        );
    }
}

#[test]
fn material_edited_mid_play_reaches_the_solver() {
    let d = PhysicsMaterial::default();
    let shape = ColliderShape::Sphere { radius: 0.5 };
    let (mut scene, id) = floor_and_body(shape, Vec3::new(0.0, 0.5, 0.0), d, d);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    scene.world.collider_mut(id).unwrap().material =
        material(0.9, 1.0, CombineMode::Maximum, CombineMode::Maximum);
    world.step(&mut scene, DT);
    let handle = world.id_to_collider[&id];
    let c = &world.colliders[handle];
    assert_eq!((c.friction(), c.restitution()), (0.9, 1.0));
    use rapier3d::prelude::CoefficientCombineRule::Max;
    assert_eq!(
        (c.friction_combine_rule(), c.restitution_combine_rule()),
        (Max, Max)
    );
}
