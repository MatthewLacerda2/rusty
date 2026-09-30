//! src/physics/compound_tests.rs — world placement and compound colliders (#445).
//!
//! Assert *signed* world positions (which side of the parent a child lands on,
//! which way a write-back moves it), so a `+`→`-` or a dropped parent in the
//! world↔local conversion is caught, not just a distance.

use glam::{Quat, Vec3};

use super::compound::{plan, relative_pose, world_pose, world_to_local, WorldPose};
use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::scene::Scene;

const DT: f32 = 0.5;
const DOWN: Vec3 = Vec3::NEG_Y;

/// An entity at local `pos` under `parent`, with an optional unit box collider.
fn add(scene: &mut Scene, parent: Option<u32>, pos: Vec3, collider: bool) -> u32 {
    let id = scene.add_entity("E".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    if let Some(p) = parent {
        scene.set_parent(id, Some(p)).unwrap();
    }
    if collider {
        let shape = ColliderShape::Box { size: Vec3::ONE };
        let c = ColliderComponent {
            active: true,
            shape,
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        };
        scene.world.set_collider(id, Some(c));
    }
    id
}

/// Give `id` a gravity-free Rigidbody (dynamic unless `kinematic`).
fn rigidbody(scene: &mut Scene, id: u32, kinematic: bool, velocity: Vec3) {
    let rb = RigidBodyComponent {
        active: true,
        is_kinematic: kinematic,
        mass: 1.0,
        velocity,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: Default::default(),
    };
    scene.world.set_rigidbody(id, Some(rb));
}

/// The entity a straight-down ray from 5 m above `(x, _, z)` hits, if any.
fn hit_below(world: &PhysicsWorld, x: f32, z: f32) -> Option<u32> {
    world
        .cast_ray(Vec3::new(x, 5.0, z), DOWN, 20.0)
        .map(|(id, _)| id)
}

fn pos(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

#[test]
fn child_collider_sits_at_its_world_position() {
    // The issue's failing case: parent at x=10, child collider at local x=1.
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::new(10.0, 0.0, 0.0), false);
    let child = add(&mut scene, Some(parent), Vec3::X, true);
    let world = PhysicsWorld::from_scene(&scene);
    assert_eq!(hit_below(&world, 11.0, 0.0), Some(child));
    assert_eq!(hit_below(&world, 1.0, 0.0), None, "not at its local pose");
    assert_eq!(hit_below(&world, 9.0, 0.0), None, "offset is +x, not -x");
}

#[test]
fn rotated_parent_turns_the_child_offset() {
    // A +90° yaw maps local +x to world -z.
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::new(10.0, 0.0, 0.0), false);
    scene.world.transform_mut(parent).unwrap().rotation = Quat::from_rotation_y(90f32.to_radians());
    let child = add(&mut scene, Some(parent), Vec3::X * 3.0, true);
    let world = PhysicsWorld::from_scene(&scene);
    assert_eq!(hit_below(&world, 10.0, -3.0), Some(child));
    assert_eq!(hit_below(&world, 10.0, 3.0), None);
}

#[test]
fn collider_without_rigidbody_joins_the_ancestor_body() {
    // Dynamic parent (own collider) + a grandchild hitbox via a bare pivot: one
    // body, two colliders; hits report the part, and the part rides the body.
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::new(10.0, 0.0, 0.0), true);
    rigidbody(&mut scene, parent, false, Vec3::new(0.0, 0.0, 2.0));
    let pivot = add(&mut scene, Some(parent), Vec3::X * 2.0, false);
    let part = add(&mut scene, Some(pivot), Vec3::X * 2.0, true);
    let mut world = PhysicsWorld::from_scene(&scene);
    assert_eq!(plan(&scene).get(&parent), Some(&vec![parent, part]));
    assert_eq!(world.id_to_body.len(), 1, "one compound body");
    assert_eq!(hit_below(&world, 14.0, 0.0), Some(part));
    assert_eq!(hit_below(&world, 10.0, 0.0), Some(parent));
    world.step(&mut scene, DT); // the body moves +1 on z
    assert_eq!(
        hit_below(&world, 14.0, 1.0),
        Some(part),
        "part moved with body"
    );
    assert!((pos(&scene, parent) - Vec3::new(10.0, 0.0, 1.0)).length() < 1e-4);
    assert_eq!(
        pos(&scene, part),
        Vec3::X * 2.0,
        "the part's local is untouched"
    );
}

#[test]
fn moving_or_deactivating_a_part_updates_its_collider() {
    // A part the animator/script moves mid-play drags its collider along the
    // body; a deactivated part's collider is disabled (no contacts). Queries
    // don't consult rapier's enabled flag, so assert the flag itself.
    let mut scene = Scene::new();
    let body = add(&mut scene, None, Vec3::ZERO, false);
    rigidbody(&mut scene, body, true, Vec3::ZERO);
    let part = add(&mut scene, Some(body), Vec3::X * 2.0, true);
    let mut world = PhysicsWorld::from_scene(&scene);
    scene.world.transform_mut(part).unwrap().position = Vec3::new(-3.0, 0.0, 0.0);
    world.step(&mut scene, DT);
    assert_eq!(hit_below(&world, -3.0, 0.0), Some(part));
    assert_eq!(hit_below(&world, 2.0, 0.0), None, "left its old offset");
    assert!(world.colliders[world.id_to_collider[&part]].is_enabled());
    scene.world.set_active(part, false);
    world.step(&mut scene, DT);
    let handle = world.id_to_collider[&part];
    assert!(!world.colliders[handle].is_enabled(), "inactive part");
}

#[test]
fn dynamic_child_writes_back_a_local_pose() {
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::new(10.0, 0.0, 0.0), false);
    let child = add(&mut scene, Some(parent), Vec3::X, true);
    rigidbody(&mut scene, child, false, Vec3::new(2.0, 0.0, 0.0));
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT); // world x 11 -> 12, so local x 1 -> 2
    assert!((pos(&scene, child) - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);
    // Unity: a moving parent does not carry a dynamic child — the body keeps
    // its world pose, so the child's local shifts by the opposite of the move.
    scene.world.rigidbody_mut(child).unwrap().velocity = Vec3::ZERO;
    scene.world.transform_mut(parent).unwrap().position = Vec3::new(20.0, 0.0, 0.0);
    world.step(&mut scene, DT);
    assert!((pos(&scene, child) - Vec3::new(-8.0, 0.0, 0.0)).length() < 1e-4);
    assert_eq!(hit_below(&world, 12.0, 0.0), Some(child));
}

#[test]
fn kinematic_child_follows_its_moving_parent() {
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::new(10.0, 0.0, 0.0), false);
    let child = add(&mut scene, Some(parent), Vec3::X, true);
    rigidbody(&mut scene, child, true, Vec3::ZERO);
    let mut world = PhysicsWorld::from_scene(&scene);
    scene.world.transform_mut(parent).unwrap().position = Vec3::new(20.0, 0.0, 0.0);
    world.step(&mut scene, DT);
    assert_eq!(hit_below(&world, 21.0, 0.0), Some(child));
    assert!((pos(&scene, child) - Vec3::X).length() < 1e-4, "local kept");
}

#[test]
fn reparenting_rebuilds_the_compound() {
    let mut scene = Scene::new();
    let body = add(&mut scene, None, Vec3::ZERO, true);
    rigidbody(&mut scene, body, true, Vec3::ZERO);
    let loose = add(&mut scene, None, Vec3::new(5.0, 0.0, 0.0), true);
    let mut world = PhysicsWorld::from_scene(&scene);
    assert_eq!(world.id_to_body.len(), 2);
    scene.set_parent(loose, Some(body)).unwrap();
    world.step(&mut scene, DT);
    assert_eq!(world.id_to_body.len(), 1, "loose joined body's rigidbody");
    assert_eq!(hit_below(&world, 5.0, 0.0), Some(loose));
}

#[test]
fn pose_helpers_invert_each_other_under_a_scaled_rotated_parent() {
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::new(1.0, 2.0, 3.0), false);
    let rot = Quat::from_rotation_z(0.5);
    let mut t = scene.world.transform_mut(parent).unwrap();
    t.rotation = rot;
    t.scale = Vec3::splat(2.0);
    drop(t);
    let child = add(&mut scene, Some(parent), Vec3::new(1.0, 0.0, 0.0), false);
    let wp = world_pose(&scene, child).unwrap();
    assert!((wp.pos - (Vec3::new(1.0, 2.0, 3.0) + rot * Vec3::X * 2.0)).length() < 1e-5);
    assert!((wp.scale - Vec3::splat(2.0)).length() < 1e-5);
    let (lp, lr) = world_to_local(&scene, child, wp.pos, wp.rot);
    assert!((lp - Vec3::X).length() < 1e-5 && lr.angle_between(Quat::IDENTITY) < 1e-4);
    let owner = world_pose(&scene, parent).unwrap();
    let (rp, _) = relative_pose(&owner, &wp);
    assert!(
        (rp - Vec3::X * 2.0).length() < 1e-5,
        "offset in world units: {rp}"
    );
}

#[test]
fn child_scale_multiplies_the_parent_scale() {
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::ZERO, false);
    scene.world.transform_mut(parent).unwrap().scale = Vec3::splat(2.0);
    let child = add(&mut scene, Some(parent), Vec3::X, false);
    scene.world.transform_mut(child).unwrap().scale = Vec3::splat(3.0);
    let wp = world_pose(&scene, child).unwrap();
    assert!(
        (wp.scale - Vec3::splat(6.0)).length() < 1e-5,
        "{}",
        wp.scale
    );
}

#[test]
fn world_to_local_unrotates_by_the_parent() {
    // A world-identity rotation under a +90° yaw parent is a -90° local yaw,
    // which maps +x to +z (a +90° yaw would give -z, a half-turn +45° neither).
    let mut scene = Scene::new();
    let parent = add(&mut scene, None, Vec3::ZERO, false);
    scene.world.transform_mut(parent).unwrap().rotation = Quat::from_rotation_y(90f32.to_radians());
    let child = add(&mut scene, Some(parent), Vec3::X, false);
    let (_, lr) = world_to_local(&scene, child, Vec3::ZERO, Quat::IDENTITY);
    assert!((lr * Vec3::X - Vec3::Z).length() < 1e-5, "{}", lr * Vec3::X);
}

#[test]
fn relative_pose_carries_the_rotational_offset() {
    // Owner yawed +90°, part yawed +90° then pitched +90° about x: relative to
    // the owner the part is only the pitch, which maps +y to +z.
    let yaw = Quat::from_rotation_y(90f32.to_radians());
    let pitch = Quat::from_rotation_x(90f32.to_radians());
    let owner = WorldPose {
        pos: Vec3::ZERO,
        rot: yaw,
        scale: Vec3::ONE,
    };
    let part = WorldPose {
        pos: Vec3::X,
        rot: yaw * pitch,
        scale: Vec3::ONE,
    };
    let (rp, rr) = relative_pose(&owner, &part);
    assert!(
        (rp - Vec3::Z).length() < 1e-5,
        "+x is +z in the owner frame: {rp}"
    );
    assert!((rr * Vec3::Y - Vec3::Z).length() < 1e-5, "{}", rr * Vec3::Y);
    assert!((rr * Vec3::X - Vec3::X).length() < 1e-5, "{}", rr * Vec3::X);
}

#[test]
fn kinematic_sweep_starts_from_the_collider_offset() {
    // A kinematic body whose only collider is a part 2 m ahead on +x: driven
    // 3 m toward a wall at x=4.5, the part (not the body origin) hits it.
    let mut scene = Scene::new();
    let body = add(&mut scene, None, Vec3::ZERO, false);
    rigidbody(&mut scene, body, true, Vec3::ZERO);
    add(&mut scene, Some(body), Vec3::X * 2.0, true);
    add(&mut scene, None, Vec3::new(4.5, 0.0, 0.0), true); // wall face at x=4
    let mut world = PhysicsWorld::from_scene(&scene);
    scene.world.transform_mut(body).unwrap().position = Vec3::X * 3.0;
    world.step(&mut scene, DT);
    let x = pos(&scene, body).x;
    assert!(
        x > 1.0 && x < 1.6,
        "part stops at the wall face: body x {x}"
    );
}
