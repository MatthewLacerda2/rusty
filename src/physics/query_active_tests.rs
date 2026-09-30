//! src/physics/query_active_tests.rs — queries skip inactive entities (#521).
//!
//! rapier's query pipeline ignores the enabled flag, so every query filters
//! through `query::is_live`. Each test runs both directions — deactivate → miss,
//! reactivate → hit — so a filter that always rejects or never rejects is caught.

use glam::Vec3;

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// An entity at local `pos` under `parent` with a `size` box collider.
fn add_box(scene: &mut Scene, parent: Option<u32>, pos: Vec3, size: Vec3) -> u32 {
    let id = scene.add_entity("E".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    if let Some(p) = parent {
        scene.set_parent(id, Some(p)).unwrap();
    }
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// Give `id` a gravity-free kinematic Rigidbody, making it a compound owner.
fn kinematic(scene: &mut Scene, id: u32) {
    let rb = RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: Default::default(),
    };
    scene.world.set_rigidbody(id, Some(rb));
}

/// Flip `id`'s activation and run the physics tick that pushes it to rapier.
fn set_active(scene: &mut Scene, world: &mut PhysicsWorld, id: u32, active: bool) {
    scene.world.set_active(id, active);
    world.step(scene, DT);
}

/// The entity a straight-down ray from 5 m above `(x, _, 0)` hits, if any.
fn hit_below(world: &PhysicsWorld, x: f32) -> Option<u32> {
    let origin = Vec3::new(x, 5.0, 0.0);
    world
        .cast_ray_filtered(origin, Vec3::NEG_Y, 20.0, |_| true)
        .map(|(id, _)| id)
}

/// Every query family's answer about a unit box `target` at the origin, as
/// seen from outside it (casts from -z, volumes and points at its center).
fn answers(world: &PhysicsWorld, target: u32) -> [bool; 9] {
    let from = Vec3::new(0.0, 0.0, -5.0);
    let only = |id: u32| id == target;
    [
        world.cast_ray(from, Vec3::Z, 20.0).is_some(),
        world.cast_ray_filtered(from, Vec3::Z, 20.0, only).is_some(),
        world.cast_ray_with_normal(from, Vec3::Z, 20.0).is_some(),
        world
            .cast_sphere_filtered(from, Vec3::Z, 0.2, 20.0, only)
            .is_some(),
        world.overlap_sphere(Vec3::ZERO, 0.2, only) == [target],
        world.overlap_box(Vec3::ZERO, Vec3::splat(0.2), only) == [target],
        world.overlap_capsule(Vec3::ZERO, Vec3::Y * 0.1, 0.2, only) == [target],
        world.closest_point_on(target, Vec3::X * 3.0).is_some(),
        world.collider_contains_point(target, Vec3::ZERO) == Some(true),
    ]
}

#[test]
fn every_query_family_skips_an_inactive_entity_and_sees_it_again() {
    let mut scene = Scene::new();
    let target = add_box(&mut scene, None, Vec3::ZERO, Vec3::ONE);
    let mut world = PhysicsWorld::from_scene(&scene);
    assert_eq!(
        answers(&world, target),
        [true; 9],
        "active: every query hits"
    );
    set_active(&mut scene, &mut world, target, false);
    assert_eq!(
        answers(&world, target),
        [false; 9],
        "inactive: every query misses"
    );
    set_active(&mut scene, &mut world, target, true);
    assert_eq!(
        answers(&world, target),
        [true; 9],
        "reactivated: hits again"
    );
}

#[test]
fn an_entity_inactive_at_build_is_hidden_until_activated() {
    let mut scene = Scene::new();
    let target = add_box(&mut scene, None, Vec3::ZERO, Vec3::ONE);
    scene.world.set_active(target, false);
    let mut world = PhysicsWorld::from_scene(&scene);
    assert_eq!(answers(&world, target), [false; 9], "built inactive");
    set_active(&mut scene, &mut world, target, true);
    assert_eq!(
        answers(&world, target),
        [true; 9],
        "its own collider re-enabled"
    );
}

#[test]
fn a_deactivated_part_is_skipped_while_its_owner_still_answers() {
    let mut scene = Scene::new();
    let owner = add_box(&mut scene, None, Vec3::ZERO, Vec3::ONE);
    kinematic(&mut scene, owner);
    let part = add_box(&mut scene, Some(owner), Vec3::X * 3.0, Vec3::ONE);
    let mut world = PhysicsWorld::from_scene(&scene);
    set_active(&mut scene, &mut world, part, false);
    assert_eq!(hit_below(&world, 3.0), None, "inactive part is missed");
    assert_eq!(hit_below(&world, 0.0), Some(owner), "owner still hit");
    assert_eq!(world.closest_point_on(part, Vec3::ZERO), None);
    set_active(&mut scene, &mut world, part, true);
    assert_eq!(hit_below(&world, 3.0), Some(part), "reactivated part hit");
}

#[test]
fn a_deactivated_owner_hides_its_active_parts() {
    // The owner's body is disabled as a whole, so a part whose own entity is
    // still active answers no query either.
    let mut scene = Scene::new();
    let owner = add_box(&mut scene, None, Vec3::ZERO, Vec3::ONE);
    kinematic(&mut scene, owner);
    let part = add_box(&mut scene, Some(owner), Vec3::X * 3.0, Vec3::ONE);
    let mut world = PhysicsWorld::from_scene(&scene);
    set_active(&mut scene, &mut world, owner, false);
    assert_eq!(hit_below(&world, 0.0), None, "owner missed");
    assert_eq!(hit_below(&world, 3.0), None, "part rides the disabled body");
    assert!(world
        .overlap_sphere(Vec3::X * 3.0, 0.2, |_| true)
        .is_empty());
    set_active(&mut scene, &mut world, owner, true);
    assert_eq!(hit_below(&world, 0.0), Some(owner));
    assert_eq!(hit_below(&world, 3.0), Some(part));
}

#[test]
fn the_character_sweep_walks_through_an_inactive_wall() {
    // A collider-only (implicit kinematic) mover scripted 4 m through a thin
    // static wall: blocked while the wall is active, straight through when the
    // wall is deactivated in the same tick as the move.
    let run = |wall_active: bool| {
        let mut scene = Scene::new();
        let mover = add_box(&mut scene, None, Vec3::ZERO, Vec3::splat(0.5));
        let wall = add_box(&mut scene, None, Vec3::X * 2.0, Vec3::new(0.2, 4.0, 4.0));
        scene.world.set_static(wall, true);
        let mut world = PhysicsWorld::from_scene(&scene);
        scene.world.set_active(wall, wall_active);
        scene.world.transform_mut(mover).unwrap().position = Vec3::X * 4.0;
        world.step(&mut scene, DT);
        let x = scene.world.transform(mover).unwrap().position.x;
        x
    };
    assert!(run(true) < 2.0, "active wall blocks: x = {}", run(true));
    assert!(
        (run(false) - 4.0).abs() < 1e-3,
        "inactive wall: x = {}",
        run(false)
    );
}
