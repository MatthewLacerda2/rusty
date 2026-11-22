use glam::{Quat, Vec3};

use crate::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_scene::skeleton::rig::armature_owner;
use rusty_scene::Scene;

pub(super) const DT: f32 = 1.0 / 60.0;

pub(super) fn collider() -> ColliderComponent {
    let shape = ColliderShape::Box {
        size: Vec3::splat(0.2),
    };
    ColliderComponent {
        shape,
        ..rusty_scene::create::default_collider()
    }
}

pub(super) fn rig(scene: &mut Scene, hitboxes: bool) -> u32 {
    let rig = armature_owner(scene);
    scene.sync_skeletons();
    for name in ["hips", "spine", "head"].into_iter().filter(|_| hitboxes) {
        let bone = scene.find_bone(rig, name).unwrap();
        let hb = scene.add_entity("Hitbox".to_string());
        scene.set_parent(hb, Some(bone)).unwrap();
        scene.world.set_collider(hb, Some(collider()));
    }
    rig
}

pub(super) fn tick(physics: &mut PhysicsWorld, scene: &mut Scene) {
    physics.step(scene, DT);
    scene.build_skin_palettes();
    physics.follow_bones(scene);
}

fn walks(physics: &PhysicsWorld) -> u64 {
    physics.tracked.follow.as_ref().map_or(0, |k| k.walks)
}

fn swing(scene: &mut Scene, rig: u32, angle: f32) {
    let spine = scene.find_bone(rig, "spine").unwrap();
    scene.world.transform_mut(spine).unwrap().rotation = Quat::from_rotation_z(angle);
}

#[test]
fn an_animating_rig_walks_its_skeleton_once() {
    for hitboxes in [false, true] {
        let mut scene = Scene::new();
        let rig = rig(&mut scene, hitboxes);
        let mut physics = PhysicsWorld::from_scene(&scene);
        for t in 0..10 {
            swing(&mut scene, rig, t as f32 * 0.1);
            tick(&mut physics, &mut scene);
        }
        assert_eq!(walks(&physics), 1, "the first follow only ({hitboxes})");
        let kept = physics.tracked.follow.as_ref().unwrap().ids().len();
        assert_eq!(kept, if hitboxes { 3 } else { 0 });
    }
}

#[test]
fn a_moved_hitbox_still_follows_its_bone_off_the_kept_list() {
    let mut scene = Scene::new();
    let rig = rig(&mut scene, true);
    let mut physics = PhysicsWorld::from_scene(&scene);
    tick(&mut physics, &mut scene);
    swing(&mut scene, rig, 1.0);
    scene.build_skin_palettes();
    physics.follow_bones(&scene);
    assert_eq!(walks(&physics), 1, "a pose is no reason to walk");
    let spine = scene.find_bone(rig, "spine").unwrap();
    let hb = scene
        .world
        .children(spine)
        .into_iter()
        .find(|&c| scene.world.has_collider(c));
    let hb = hb.unwrap();
    let at = physics.colliders[physics.id_to_collider[&hb]]
        .position()
        .translation;
    let want = scene.world.world_pose(hb).unwrap().pos;
    assert!((at - want).length() < 1e-5, "{at:?} vs {want:?}");
}

#[test]
fn a_rewound_world_and_a_new_hitbox_walk_again() {
    let mut scene = Scene::new();
    let rig = rig(&mut scene, true);
    let mut physics = PhysicsWorld::from_scene(&scene);
    tick(&mut physics, &mut scene);
    // As an Undo, a scene load and Play's restore do.
    scene.world.rewind_changes();
    tick(&mut physics, &mut scene);
    assert_eq!(walks(&physics), 2, "a Rebuild walks");
    tick(&mut physics, &mut scene);
    assert_eq!(walks(&physics), 2, "and the tick after is quiet again");
    let head = scene.find_bone(rig, "head").unwrap();
    let hb = scene.add_entity("Visor".to_string());
    scene.set_parent(hb, Some(head)).unwrap();
    scene.world.set_collider(hb, Some(collider()));
    tick(&mut physics, &mut scene);
    assert!(physics.tracked.follow.as_ref().unwrap().ids().contains(&hb));
}

#[test]
fn a_write_to_a_characters_mesh_walks_only_when_its_slots_move() {
    let mut scene = Scene::new();
    let rig = rig(&mut scene, true);
    let mut physics = PhysicsWorld::from_scene(&scene);
    tick(&mut physics, &mut scene);
    scene.world.mesh_mut(rig).unwrap().max_draw_distance = 40.0;
    tick(&mut physics, &mut scene);
    assert_eq!(walks(&physics), 1, "a draw distance moves no bone");
    scene.world.mesh_mut(rig).unwrap().skeleton.bones[2] = None;
    tick(&mut physics, &mut scene);
    assert_eq!(walks(&physics), 2, "a slot cleared");
}
