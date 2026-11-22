use glam::Vec3;

use super::super::world::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, JointComponent, RigidBodyComponent};
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug)]
enum Own {
    Fixed,
    Dynamic,
    Kinematic,
}

fn body(scene: &mut Scene, pos: Vec3, own: Own) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..rusty_scene::create::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: matches!(own, Own::Kinematic),
        ..rusty_scene::create::defaults::default_rigidbody()
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
    let (mut scene, mut physics, _) = jointed(Own::Dynamic, true);
    step(&mut scene, &mut physics, 1);
    let a = scene.world.ids_with_joint()[0];
    scene.world.set_static(a, true);
    step(&mut scene, &mut physics, 3);
    assert_eq!(physics.joints.len(), 0);
}

#[test]
fn deactivating_the_owner_of_an_inactive_connected_part_drops_the_joint() {
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
    // The part is inactive, its owner's body enabled: the joint is built. Nothing moves,
    // so no pose stamp re-runs the joint sync by chance once the owner goes.
    let mut physics = PhysicsWorld::from_scene(&scene);
    step(&mut scene, &mut physics, 2);
    assert_eq!(physics.joints.len(), 1, "built to the enabled body");
    scene.world.set_active(owner, false);
    step(&mut scene, &mut physics, 1);
    assert_eq!(physics.joints.len(), 0, "dropped with its body");
}
