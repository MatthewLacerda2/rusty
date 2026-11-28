//! A floating-origin shift (`Scene::shift_origin`) is a cut: bodies land where their
//! Transforms moved, and near zero they keep a course they lose 30 km out.

use glam::Vec3;

use super::cut_tests::{drifting, moved, DT};
use crate::PhysicsWorld;
use rusty_scene::create::{default_collider, default_rigidbody};
use rusty_scene::Scene;

/// How far a box resting on a floor at `x` strays over 600 ticks, the World shifted by
/// `shift` steps before the first.
fn resting_drift(x: f32, shift: i32) -> f32 {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    let size = Vec3::new(40.0, 1.0, 40.0);
    let shape = rusty_components::ColliderShape::Box { size };
    scene.world.set_collider(
        floor,
        Some(rusty_components::ColliderComponent {
            shape,
            ..default_collider()
        }),
    );
    moved(&mut scene, floor, Vec3::new(x, -0.5, 0.0));
    scene.world.set_static(floor, true);
    let id = scene.add_entity("Box".to_string());
    scene.world.set_collider(id, Some(default_collider()));
    scene.world.set_rigidbody(id, Some(default_rigidbody()));
    moved(&mut scene, id, Vec3::new(x + 0.3, 0.5, 0.2));
    let mut physics = PhysicsWorld::from_scene(&scene);
    scene.shift_origin(glam::IVec3::new(shift, 0, 0));
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    let start = scene.world.transform(id).unwrap().position;
    let mut drift: f32 = 0.0;
    for _ in 0..600 {
        physics.step(&mut scene, DT);
        let at = scene.world.transform(id).unwrap().position;
        drift = drift.max(at.distance(start));
    }
    drift
}

#[test]
fn a_shift_lands_a_resting_body_on_its_floor() {
    assert!(
        resting_drift(1024.0, 1) < 1e-3,
        "shifted to zero, it stays put"
    );
}

/// Metres off its straight course, at worst, a body gliding at 0.7 m/s 30 km out strays.
fn course_miss(shift: i32) -> f32 {
    let (mut scene, _, id) = drifting();
    scene.world.rigidbody_mut(id).unwrap().velocity = Vec3::new(0.7, 0.0, 0.0);
    moved(&mut scene, id, Vec3::new(30_720.0, 0.0, 0.0));
    let mut physics = PhysicsWorld::from_scene(&scene);
    scene.shift_origin(glam::IVec3::new(shift, 0, 0));
    let start = scene.world.transform(id).unwrap().position.x;
    let mut worst: f32 = 0.0;
    for n in 1..=600 {
        physics.step(&mut scene, DT);
        let at = scene.world.transform(id).unwrap().position.x;
        worst = worst.max((at - (start + 0.7 * DT * n as f32)).abs());
    }
    worst
}

#[test]
fn far_out_a_slow_body_strays_off_course_and_shifted_it_keeps_it() {
    let (off, on) = (course_miss(0), course_miss(30));
    assert!(
        on < 1e-3,
        "shifted to zero: within a millimetre, missed by {on} m"
    );
    assert!(
        off > 0.1,
        "30 km out every step rounds to 2 mm: missed by {off} m"
    );
}
