//! src/physics/collision_tests.rs — solid-contact events (#448): a sphere dropped
//! onto a floor enters, stays and exits exactly once, reports the contact point
//! and normal a script expects, and replays identically.

use glam::Vec3;

use super::{CollisionEvents, PhysicsWorld};
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn collider(shape: ColliderShape) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape,
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

/// A static floor with its top face at `y = 0` and a dynamic unit-radius
/// sphere 2 units above it. Returns `(scene, floor, ball)`.
fn sphere_over_floor() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    scene.world.transform_mut(floor).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
    scene.world.set_static(floor, true);
    let size = Vec3::new(20.0, 1.0, 20.0);
    scene
        .world
        .set_collider(floor, Some(collider(ColliderShape::Box { size })));
    let ball = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(ball).unwrap().position = Vec3::new(0.0, 2.0, 0.0);
    let sphere = ColliderShape::Sphere { radius: 1.0 };
    scene.world.set_collider(ball, Some(collider(sphere)));
    scene.world.set_rigidbody(
        ball,
        Some(RigidBodyComponent {
            is_kinematic: false,
            ..crate::scene::authoring::defaults::default_rigidbody()
        }),
    );
    (scene, floor, ball)
}

/// Step until the first tick with an `entered` pair; that tick's events.
fn step_until_enter(scene: &mut Scene, world: &mut PhysicsWorld) -> CollisionEvents {
    for _ in 0..120 {
        let ev = world.step(scene, DT).collisions;
        if !ev.entered.is_empty() {
            return ev;
        }
    }
    panic!("the sphere never landed");
}

#[test]
fn dropped_sphere_enters_stays_and_exits_once() {
    let (mut scene, floor, ball) = sphere_over_floor();
    let key = (floor.min(ball), floor.max(ball));
    let mut world = PhysicsWorld::from_scene(&scene);

    let ev = step_until_enter(&mut scene, &mut world);
    assert_eq!(
        ev.entered.iter().map(|p| p.key()).collect::<Vec<_>>(),
        [key]
    );
    assert_eq!(ev.stayed.len(), 1, "stay covers the enter tick");

    for _ in 0..30 {
        let ev = world.step(&mut scene, DT).collisions;
        assert!(ev.entered.is_empty(), "enter fires once per contact");
        assert_eq!(ev.stayed.len(), 1, "a resting sphere stays in contact");
    }

    // Launch it upward: the tick it separates exits once, then silence.
    scene.world.rigidbody_mut(ball).unwrap().velocity = Vec3::new(0.0, 20.0, 0.0);
    let ev = world.step(&mut scene, DT).collisions;
    assert!(ev.stayed.is_empty(), "launched away, no contact remains");
    assert_eq!(ev.exited, vec![key], "separation exits once");
    assert!(world.step(&mut scene, DT).collisions.is_empty());
}

#[test]
fn landing_contact_reports_point_normal_and_impact() {
    let (mut scene, floor, ball) = sphere_over_floor();
    let mut world = PhysicsWorld::from_scene(&scene);
    let ev = step_until_enter(&mut scene, &mut world);
    let side = ev.entered[0]
        .sides()
        .into_iter()
        .find(|(id, _, _)| *id == ball)
        .unwrap();
    let (_, other, c) = side;
    assert_eq!(other, floor);
    assert!(c.point.x.abs() < 1e-3 && c.point.z.abs() < 1e-3, "{c:?}");
    assert!(
        c.point.y.abs() < 0.1,
        "contact sits on the floor's top face: {c:?}"
    );
    assert!(
        (c.normal - Vec3::Y).length() < 1e-3,
        "the ball sees the floor push up"
    );
    // The floor is still, the ball falls: the floor closes on the ball upward.
    assert!(c.relative_velocity.y > 1.0, "{c:?}");
    assert!(c.impulse > 0.0, "the solver stopped the fall: {c:?}");
    assert_eq!(c.other_body, floor, "a static floor is its own body");
}

#[test]
fn contact_events_replay_identically() {
    let run = || {
        let (mut scene, _, _) = sphere_over_floor();
        let mut world = PhysicsWorld::from_scene(&scene);
        (0..90)
            .map(|_| format!("{:?}", world.step(&mut scene, DT).collisions))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run(), "same inputs, same contact stream");
}

/// A pair keyed `(1, 2)` whose solver impulse this tick is `impulse`.
fn pair(impulse: f32) -> super::CollisionPair {
    let contact = super::Contact {
        point: Vec3::ZERO,
        normal: Vec3::Y,
        relative_velocity: Vec3::ZERO,
        impulse,
        other_body: 2,
    };
    super::CollisionPair {
        a: 1,
        b: 2,
        contact,
        body_a: 1,
    }
}

#[test]
fn a_pair_enters_on_impulse_and_stays_while_touching() {
    let ev = CollisionEvents::from_contact_sets(&[], vec![pair(0.0)]);
    assert!(ev.is_empty(), "touching with no push yet has not collided");
    let ev = CollisionEvents::from_contact_sets(&[], vec![pair(2.0)]);
    assert_eq!((ev.entered.len(), ev.stayed.len()), (1, 1));
    let ev = CollisionEvents::from_contact_sets(&[(1, 2)], vec![pair(0.0)]);
    assert!(
        ev.entered.is_empty() && ev.stayed.len() == 1,
        "once in, touch keeps it"
    );
}
