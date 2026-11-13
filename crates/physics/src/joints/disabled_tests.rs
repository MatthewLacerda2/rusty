//! crates/physics/src/joints/disabled_tests.rs — a `Joint` whose body is disabled (its
//! owner inactive) is inert, not a crash (#1775): rapier selects joints for its
//! solver without checking `is_enabled`, and a fixed body jointed to one built
//! disabled reaches `unreachable!()` in its solver grouping.

use glam::Vec3;

use super::super::world::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, JointComponent, RigidBodyComponent};
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// What a box's body is.
#[derive(Clone, Copy, Debug)]
enum Own {
    Fixed,
    Dynamic,
    Kinematic,
}

/// A unit box with a Rigidbody at `pos`.
fn body(scene: &mut Scene, pos: Vec3, own: Own) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..rusty_scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: matches!(own, Own::Kinematic),
        ..rusty_scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    scene.world.set_static(id, matches!(own, Own::Fixed));
    id
}

fn step(scene: &mut Scene, world: &mut PhysicsWorld, n: usize) {
    for _ in 0..n {
        world.step(scene, DT);
    }
}

/// An `own` box jointed to a dynamic box under a parent, the parent inactive
/// before physics is built when `built_disabled` (so the connected body is built
/// disabled) and deactivated after a step otherwise. Returns the scene, its
/// physics and the parent.
fn jointed(own: Own, built_disabled: bool) -> (Scene, PhysicsWorld, u32) {
    let mut scene = Scene::new();
    let a = body(&mut scene, Vec3::new(0.0, 5.0, 0.0), own);
    let parent = scene.add_entity("Parent".to_string());
    let b = body(&mut scene, Vec3::new(0.0, 7.0, 0.0), Own::Dynamic);
    scene.set_parent(b, Some(parent)).unwrap();
    let j = JointComponent {
        connected_body: Some(b),
        ..Default::default()
    };
    scene.world.set_joint(a, Some(j));
    scene.world.set_active(parent, !built_disabled);
    let mut physics = PhysicsWorld::from_scene(&scene);
    if !built_disabled {
        step(&mut scene, &mut physics, 2);
        assert_eq!(physics.joints.len(), 1, "{own:?}: built while active");
        scene.world.set_active(parent, false);
    }
    (scene, physics, parent)
}

#[test]
fn a_joint_to_a_disabled_body_is_inert_and_comes_back_when_it_is_enabled() {
    for own in [Own::Fixed, Own::Dynamic, Own::Kinematic] {
        for built_disabled in [true, false] {
            let at = format!("{own:?}, built disabled: {built_disabled}");
            let (mut scene, mut physics, parent) = jointed(own, built_disabled);
            step(&mut scene, &mut physics, 3);
            assert_eq!(physics.joints.len(), 0, "{at}: inert");
            scene.world.set_active(parent, true);
            step(&mut scene, &mut physics, 1);
            assert_eq!(physics.joints.len(), 1, "{at}: back");
        }
    }
}

#[test]
fn a_joint_whose_body_turns_fixed_beside_a_disabled_one_steps() {
    // The issue's order: the joint is built from a dynamic box, then the box
    // turns static while its partner is disabled.
    let (mut scene, mut physics, _) = jointed(Own::Dynamic, true);
    step(&mut scene, &mut physics, 1);
    let a = scene.world.ids_with_joint()[0];
    scene.world.set_static(a, true);
    step(&mut scene, &mut physics, 3);
    assert_eq!(physics.joints.len(), 0);
}

#[test]
fn deactivating_the_owner_of_an_inactive_connected_part_drops_the_joint() {
    // The connected entity is an inactive collider under the body's owner: the
    // body is enabled, so the joint is built. Deactivating the owner leaves the
    // part's own activeInHierarchy unchanged, yet disables the body. Nothing
    // moves, so no pose stamp re-runs the joint sync by chance.
    let mut scene = Scene::new();
    let a = body(&mut scene, Vec3::new(0.0, 5.0, 0.0), Own::Fixed);
    let owner = body(&mut scene, Vec3::new(0.0, 7.0, 0.0), Own::Kinematic);
    let part = scene.add_entity("Part".to_string());
    scene.set_parent(part, Some(owner)).unwrap();
    scene.world.set_active(part, false);
    let j = JointComponent {
        connected_body: Some(part),
        ..Default::default()
    };
    scene.world.set_joint(a, Some(j));
    let mut physics = PhysicsWorld::from_scene(&scene);
    step(&mut scene, &mut physics, 2);
    assert_eq!(physics.joints.len(), 1, "built to the enabled body");
    scene.world.set_active(owner, false);
    step(&mut scene, &mut physics, 1);
    assert_eq!(physics.joints.len(), 0, "dropped with its body");
}
