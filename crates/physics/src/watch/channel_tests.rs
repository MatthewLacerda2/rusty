use glam::Vec3;

use super::edit_tests::{edit_one, Phys};
use crate::PhysicsWorld;
use rusty_components::CharacterControllerComponent;
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;
const BONES: usize = 16;

fn yard(n: usize) -> (Scene, PhysicsWorld, Vec<(u32, Vec<u32>)>) {
    let mut scene = Scene::new();
    let mut soldiers = Vec::new();
    for s in 0..n {
        let root = scene.add_entity(format!("soldier{s}"));
        let cc = CharacterControllerComponent::default();
        scene.world.set_character_controller(root, Some(cc));
        scene.world.transform_mut(root).unwrap().position = Vec3::new(s as f32 * 3.0, 1.0, 0.0);
        let mut hitboxes = Vec::new();
        for b in 0..BONES {
            let bone = scene.add_entity(format!("bone{s}.{b}"));
            scene.set_parent(bone, Some(root)).unwrap();
            let hitbox = scene.add_entity(format!("hitbox{s}.{b}"));
            scene.set_parent(hitbox, Some(bone)).unwrap();
            edit_one(&mut scene.world, hitbox, &Phys::Collider(1));
            let at = Vec3::new(0.0, b as f32 * 0.1, 0.0);
            scene.world.transform_mut(bone).unwrap().position = at;
            hitboxes.push(hitbox);
        }
        soldiers.push((root, hitboxes));
    }
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..3 {
        physics.step(&mut scene, DT);
    }
    (scene, physics, soldiers)
}

fn walk(scene: &mut Scene, soldiers: &[(u32, Vec<u32>)]) {
    for &(root, _) in soldiers {
        scene.world.transform_mut(root).unwrap().position.x += 0.05;
    }
}

fn step_after(physics: &mut PhysicsWorld, scene: &mut Scene) -> [usize; 6] {
    let v = &physics.tracked.visits;
    v.members.set(0);
    physics.step(scene, DT);
    let v = &physics.tracked.visits;
    let members = v.members.get() as usize;
    [members, v.activity, v.collider, v.layer, v.fits, v.every]
}

#[test]
fn a_tick_where_only_poses_moved_runs_only_the_pose_passes() {
    let (mut scene, mut physics, soldiers) = yard(50);
    walk(&mut scene, &soldiers);
    let [members, activity, collider, layer, fits, every] = step_after(&mut physics, &mut scene);
    let named = physics.tracked.visits.changed;
    assert_eq!(
        named,
        50 * (1 + 2 * BONES),
        "every soldier's subtree is stamped"
    );
    assert_eq!(members, 0, "a pure move cannot re-home a collider");
    assert_eq!([activity, collider, layer, fits], [0; 4]);
    assert_eq!(every, 50);
}

fn edit_one_hitbox(edit: impl FnOnce(&mut Scene, u32)) -> (Scene, PhysicsWorld, u32, [usize; 6]) {
    let (mut scene, mut physics, soldiers) = yard(1);
    let hitbox = soldiers[0].1[3];
    walk(&mut scene, &soldiers);
    edit(&mut scene, hitbox);
    let visits = step_after(&mut physics, &mut scene);
    (scene, physics, hitbox, visits)
}

#[test]
fn an_activation_reaches_only_the_enabled_pass() {
    let (_, physics, hitbox, v) = edit_one_hitbox(|s, h| _ = s.world.set_active(h, false));
    assert!(
        v[0] > 0,
        "an Activity stamp may re-home: the plan check reads it"
    );
    assert_eq!(v[1..], [1, 0, 0, 0, 1]);
    let c = &physics.colliders[physics.id_to_collider[&hitbox]];
    assert!(!c.is_enabled());
}

#[test]
fn a_layer_change_reaches_only_the_layer_pass() {
    let (_, physics, hitbox, v) = edit_one_hitbox(|s, h| _ = s.world.set_layer(h, 2));
    assert_eq!(v, [0, 0, 0, 1, 0, 1]);
    let c = &physics.colliders[physics.id_to_collider[&hitbox]];
    let groups = c.collision_groups().memberships.bits();
    assert_eq!(groups, 1 << 2, "the hitbox's groups follow its new layer");
}

#[test]
fn a_collider_edit_reaches_the_collider_and_fit_passes() {
    let (_, physics, hitbox, v) =
        edit_one_hitbox(|s, h| edit_one(&mut s.world, h, &Phys::Friction(2)));
    assert!(
        v[0] > 0,
        "a Collider stamp may re-home: the plan check reads it"
    );
    assert_eq!(v[1..], [0, 1, 0, 1, 1]);
    let c = &physics.colliders[physics.id_to_collider[&hitbox]];
    assert_eq!(c.friction(), 0.8);
}

#[test]
fn a_trigger_edit_reaches_the_sensor_pass() {
    let (_, physics, hitbox, v) =
        edit_one_hitbox(|s, h| edit_one(&mut s.world, h, &Phys::Trigger(true)));
    assert_eq!(v[1..], [0, 1, 0, 1, 1]);
    assert!(physics.colliders[physics.id_to_collider[&hitbox]].is_sensor());
}

#[test]
fn a_static_flag_reaches_only_the_fit_pass() {
    let (_, _, _, v) = edit_one_hitbox(|s, h| _ = s.world.set_static(h, true));
    assert_eq!(v, [0, 0, 0, 0, 1, 1]);
}
