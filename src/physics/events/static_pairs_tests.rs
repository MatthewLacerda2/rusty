//! src/physics/events/static_pairs_tests.rs — two static colliders never pair
//! (#1779): a wall standing on a floor keeps no contact and raises no event, a
//! static trigger ignores the static wall it overlaps, while a kinematic mover
//! still meets the wall and a body made non-static in Play pairs again.

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// An entity at `pos` with a `size` box collider, static when `is_static`
/// (else, with no Rigidbody, a kinematic collider).
fn boxed(scene: &mut Scene, pos: Vec3, size: Vec3, is_static: bool, trigger: bool) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, is_static);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size },
        is_trigger: trigger,
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// A static floor with its top face at `y = 0` and a 1×2×1 wall standing on it.
fn wall_on_floor(wall_static: bool) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let floor = boxed(
        &mut scene,
        -Vec3::Y * 0.5,
        Vec3::new(20.0, 1.0, 20.0),
        true,
        false,
    );
    let wall = boxed(
        &mut scene,
        Vec3::Y,
        Vec3::new(1.0, 2.0, 1.0),
        wall_static,
        false,
    );
    (scene, floor, wall)
}

/// How many collider pairs rapier holds an actual contact for.
fn touching(world: &PhysicsWorld) -> usize {
    let pairs = world.narrow_phase.contact_pairs();
    pairs.filter(|p| p.has_any_active_contact()).count()
}

#[test]
fn a_static_wall_on_a_static_floor_keeps_no_contact_or_event() {
    let (mut scene, _, _) = wall_on_floor(true);
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        let ev = world.step(&mut scene, DT);
        assert!(ev.collisions.is_empty() && ev.triggers.is_empty());
    }
    assert_eq!(touching(&world), 0, "static↔static is never solved");
}

#[test]
fn a_kinematic_wall_on_a_static_floor_still_touches() {
    let (mut scene, _, _) = wall_on_floor(false);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    world.step(&mut scene, DT);
    assert_eq!(touching(&world), 1, "kinematic↔static still pairs");
}

#[test]
fn a_wall_made_non_static_in_play_pairs_again_and_back() {
    let (mut scene, _, wall) = wall_on_floor(true);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    assert_eq!(touching(&world), 0);
    scene.world.set_static(wall, false);
    world.step(&mut scene, DT);
    world.step(&mut scene, DT);
    assert_eq!(
        touching(&world),
        1,
        "the type filter reads today's body type"
    );
    scene.world.set_static(wall, true);
    world.step(&mut scene, DT);
    world.step(&mut scene, DT);
    assert_eq!(
        touching(&world),
        0,
        "made static again, the pair is dropped"
    );
}

#[test]
fn a_static_trigger_ignores_static_walls_but_not_movers() {
    let (mut scene, floor, wall) = wall_on_floor(true);
    let zone = boxed(&mut scene, Vec3::ZERO, Vec3::splat(4.0), true, true);
    let mut world = PhysicsWorld::from_scene(&scene);
    assert!(
        world.step(&mut scene, DT).triggers.is_empty(),
        "both static"
    );
    scene.world.set_static(wall, false);
    let ev = world.step(&mut scene, DT).triggers;
    assert_eq!(ev.entered, vec![(zone.min(wall), zone.max(wall))]);
    assert!(!ev.stayed.contains(&(floor.min(zone), floor.max(zone))));
}
