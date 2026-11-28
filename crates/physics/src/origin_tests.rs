//! The one-pass shift (`prepare_shift`, then the scene's, then `shift_origin`): bodies keep
//! their course and rest, a kinematic carrier launches nothing, and the next step
//! re-reads no entity; a query before that step sees the moved world.

use glam::{IVec3, Vec3};

use crate::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty_scene::create::{default_collider, default_rigidbody};
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn shift(scene: &mut Scene, physics: &mut PhysicsWorld, steps: IVec3) {
    physics.prepare_shift(scene);
    let by = scene.shift_origin(steps);
    physics.shift_origin(scene, by);
}

fn put(
    scene: &mut Scene,
    name: &str,
    at: Vec3,
    size: Vec3,
    body: Option<RigidBodyComponent>,
) -> u32 {
    let id = scene.add_entity(name.to_string());
    let shape = ColliderShape::Box { size };
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    scene.world.transform_mut(id).unwrap().position = at;
    match body {
        Some(rb) => drop(scene.world.set_rigidbody(id, Some(rb))),
        None => drop(scene.world.set_static(id, true)),
    }
    id
}

fn at(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

#[test]
fn a_box_resting_on_a_far_floor_lands_on_it_and_the_next_step_reads_nothing() {
    let mut scene = Scene::new();
    let x = 30_720.0;
    put(
        &mut scene,
        "Floor",
        Vec3::new(x, -0.5, 0.0),
        Vec3::new(40.0, 1.0, 40.0),
        None,
    );
    let id = put(
        &mut scene,
        "Box",
        Vec3::new(x, 0.5, 0.0),
        Vec3::ONE,
        Some(default_rigidbody()),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..240 {
        physics.step(&mut scene, DT);
    }
    let before = at(&scene, id);
    shift(&mut scene, &mut physics, IVec3::new(30, 0, 0));
    // Before any step: the tick's scripts and queries already see the moved floor.
    let down = physics.cast_ray_with_normal(Vec3::new(5.0, 3.0, 0.0), -Vec3::Y, 10.0);
    assert!(
        down.is_some_and(|(d, _)| (d - 3.0).abs() < 1e-3),
        "{down:?}"
    );
    physics.step(&mut scene, DT);
    assert_eq!(
        physics.tracked.visits.changed, 0,
        "no entity re-read after the shift"
    );
    assert!((at(&scene, id) - (before - Vec3::X * x)).length() < 1e-3);
}

#[test]
fn a_glider_keeps_its_course_and_speed_through_the_shift() {
    let mut scene = Scene::new();
    let rb = RigidBodyComponent {
        velocity: Vec3::new(5.0, 0.0, 0.0),
        use_gravity: false,
        ..default_rigidbody()
    };
    let id = put(
        &mut scene,
        "Glider",
        Vec3::new(5000.0, 3.0, 0.0),
        Vec3::ONE,
        Some(rb),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    physics.step(&mut scene, DT);
    shift(&mut scene, &mut physics, IVec3::new(5, 0, 0));
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    let x = at(&scene, id).x;
    // Its one step at 5 km rounded by a fraction of a millimetre.
    assert!(
        (x - (5000.0 - 5120.0 + 5.0 * 61.0 * DT)).abs() < 5e-3,
        "x {x}"
    );
    assert_eq!(
        scene.world.rigidbody(id).unwrap().velocity,
        Vec3::new(5.0, 0.0, 0.0)
    );
}

#[test]
fn a_kinematic_carrier_launches_nothing_it_carries() {
    let mut scene = Scene::new();
    let kinematic = RigidBodyComponent {
        is_kinematic: true,
        ..default_rigidbody()
    };
    put(
        &mut scene,
        "Deck",
        Vec3::new(2000.0, -0.5, 0.0),
        Vec3::new(10.0, 1.0, 10.0),
        Some(kinematic),
    );
    let id = put(
        &mut scene,
        "Crate",
        Vec3::new(2000.0, 0.5, 0.0),
        Vec3::ONE,
        Some(default_rigidbody()),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        physics.step(&mut scene, DT);
    }
    shift(&mut scene, &mut physics, IVec3::new(2, 0, 0));
    for _ in 0..30 {
        physics.step(&mut scene, DT);
    }
    let v = scene.world.rigidbody(id).unwrap().velocity;
    assert!(v.length() < 0.05, "resting on the deck: {v}");
    assert!((at(&scene, id).y - 0.5).abs() < 0.01);
}
