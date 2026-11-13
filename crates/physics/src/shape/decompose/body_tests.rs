//! crates/physics/src/shape/decompose/body_tests.rs — dynamic bodies with concave mesh
//! colliders simulate as solids (#1886): an arch stands on both feet, a box stays
//! in a bowl, and a ball driven inside the arch is pushed out, not let through.

use glam::{Quat, Vec3};

use crate::shape::fixture::{arch, bowl, floor, prop, steps};

use crate::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty_scene::authoring::{default_collider, default_rigidbody};
use rusty_scene::Scene;

#[test]
fn a_dynamic_arch_dropped_on_a_floor_rests_on_both_feet() {
    let mut scene = Scene::new();
    floor(&mut scene);
    let id = prop(&mut scene, arch(), Vec3::new(0.0, 0.5, 0.0), false);
    let mut world = PhysicsWorld::from_scene(&scene);
    steps(&mut scene, &mut world, 180);
    let t = scene.world.transform(id).unwrap();
    assert!(t.position.y.abs() < 0.05, "feet at y = {}", t.position.y);
    let tilt = t.rotation.angle_between(Quat::IDENTITY);
    assert!(tilt < 0.02, "level on both feet, tilted {tilt} rad");
    let body = &world.bodies[world.id_to_body[&id]];
    assert!((body.mass() - 5.0).abs() < 1e-3, "mass {}", body.mass());
    // A solid's inertia, about every axis: a trimesh would have none.
    let inertia = body.mass_properties().local_mprops.principal_inertia();
    assert!(inertia.min_element() > 0.1, "inertia {inertia:?}");
}

#[test]
fn a_box_thrown_into_a_dynamic_bowl_stays_inside() {
    let mut scene = Scene::new();
    floor(&mut scene);
    let bowl_id = prop(&mut scene, bowl(), Vec3::ZERO, false);
    let crate_id = scene.add_entity("Crate".to_string());
    scene.world.transform_mut(crate_id).unwrap().position = Vec3::new(-0.4, 2.0, 0.0);
    let shape = ColliderShape::Box {
        size: Vec3::splat(0.4),
    };
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    scene.world.set_collider(crate_id, Some(collider));
    let rb = RigidBodyComponent {
        velocity: Vec3::new(3.0, -2.0, 0.0),
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(crate_id, Some(rb));
    let mut world = PhysicsWorld::from_scene(&scene);
    steps(&mut scene, &mut world, 240);
    let bowl_at = scene.world.transform(bowl_id).unwrap().position;
    let p = scene.world.transform(crate_id).unwrap().position - bowl_at;
    assert!(
        p.x.abs() < 1.0 && p.z.abs() < 1.0,
        "crate left the bowl: {p}"
    );
    assert!(
        p.y > 0.2 && p.y < 1.0,
        "crate rests on the bowl's floor: {p}"
    );
}

#[test]
fn a_fast_ball_that_ends_a_tick_inside_the_arch_is_pushed_out() {
    // 0.3 m a tick puts the ball inside the arch's 0.4 m top after one step:
    // a trimesh has only surfaces there, so it fell through to the floor.
    let mut scene = Scene::new();
    floor(&mut scene);
    prop(&mut scene, arch(), Vec3::ZERO, false);
    let ball = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(ball).unwrap().position = Vec3::new(-0.8, 1.15, 0.0);
    let shape = ColliderShape::Sphere { radius: 0.05 };
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    scene.world.set_collider(ball, Some(collider));
    let rb = RigidBodyComponent {
        velocity: Vec3::new(0.0, -18.0, 0.0),
        mass: 0.1,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(ball, Some(rb));
    let mut world = PhysicsWorld::from_scene(&scene);
    steps(&mut scene, &mut world, 30);
    let y = scene.world.transform(ball).unwrap().position.y;
    assert!(y > 0.9, "the ball went into the arch's top: y = {y}");
}

#[test]
fn a_ray_through_a_decomposed_arch_crosses_each_leg_and_the_whole_top() {
    let mut scene = Scene::new();
    let id = prop(&mut scene, arch(), Vec3::ZERO, false);
    scene.world.rigidbody_mut(id).unwrap().use_gravity = false;
    let world = PhysicsWorld::from_scene(&scene);
    let thick = |y: f32| -> Vec<f32> {
        let from = Vec3::new(-3.0, y, 0.0);
        let crossings = world.raycast_through(from, Vec3::X, 10.0, |_| true);
        crossings.iter().map(|c| c.thickness).collect()
    };
    let legs = thick(0.3);
    assert_eq!(
        legs.len(),
        2,
        "one crossing per leg, not one over the gap: {legs:?}"
    );
    assert!(legs.iter().all(|t| (t - 0.4).abs() < 0.1), "{legs:?}");
    let top = thick(0.8);
    assert_eq!(top.len(), 1, "the top's pieces read as one solid: {top:?}");
    assert!((top[0] - 2.0).abs() < 0.1, "{top:?}");
}
