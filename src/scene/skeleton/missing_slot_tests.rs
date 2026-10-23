//! A destroyed bone's slot is `None`, never a dead id (#1273): a later entity
//! given that id is not mistaken for the bone.

use super::fixture::armature_owner;
use crate::scene::{to_scene_data, Scene};

/// An incoming level of `n` plain entities, ids 1..=n.
fn level(n: usize) -> crate::scene::serialize::SceneData {
    let mut level = Scene::new();
    for i in 0..n {
        level.add_entity(format!("Crate{i}"));
    }
    to_scene_data(&level)
}

#[test]
fn a_carried_rig_never_drives_the_entity_given_its_dead_bones_id() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let hips = scene.find_bone(rig, "hips").unwrap();
    let spine = scene.find_bone(rig, "spine").unwrap();
    scene.destroy_entity(spine); // spine and head
    assert!(scene.dont_destroy_on_load(rig));

    let survivors = scene.load_survivors();
    scene
        .swap_in(level(6), "rusty_1273_no_such_dir/level.scene", &survivors)
        .unwrap();
    scene.sync_skeletons();

    assert!(
        scene.world.contains(spine),
        "the incoming level took the id"
    );
    let slots = scene.world.mesh(rig).unwrap().skeleton.bones.clone();
    assert_eq!(slots, [Some(hips), None, None]);
    assert!(!scene.is_bone(spine), "the newcomer is no bone");
    assert_eq!(scene.bone_ids().len(), 1);
    let before = scene.world.transform(spine).unwrap().position;
    scene.build_skin_palettes();
    assert_eq!(scene.world.transform(spine).unwrap().position, before);
}

#[test]
fn a_bone_moved_under_a_prop_stays_bound_and_stays_there() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let head = scene.find_bone(rig, "head").unwrap();
    let prop = scene.add_entity("Prop".to_string());
    scene.set_parent(head, Some(prop)).unwrap();

    // Any instantiate re-syncs every skeleton.
    super::fixture::hero(&mut scene, "1273_moved");

    assert_eq!(
        scene.world.parent_id(head),
        Some(prop),
        "the head stays on the prop"
    );
    let slots = scene.world.mesh(rig).unwrap().skeleton.bones.clone();
    assert_eq!(
        slots[2],
        Some(head),
        "still bound: skinned by its world matrix"
    );
    let heads = scene.entity_ids().into_iter();
    let heads = heads.filter(|&e| scene.world.name(e).is_some_and(|n| *n == "head"));
    assert_eq!(heads.count(), 1, "no head was respawned");
}

#[test]
fn a_carried_rig_never_drives_the_entity_given_a_moved_bones_id() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let head = scene.find_bone(rig, "head").unwrap();
    let prop = scene.add_entity("Prop".to_string());
    scene.set_parent(head, Some(prop)).unwrap();
    assert!(scene.dont_destroy_on_load(rig));

    let survivors = scene.load_survivors();
    assert!(!survivors.contains(&head), "the prop is not carried");
    scene
        .swap_in(level(6), "rusty_1273_no_such_dir/level.scene", &survivors)
        .unwrap();
    scene.sync_skeletons();

    assert!(scene.world.contains(head), "the incoming level took the id");
    let slots = scene.world.mesh(rig).unwrap().skeleton.bones.clone();
    assert!(!slots.contains(&Some(head)), "{slots:?}");
    assert!(!scene.is_bone(head));
    assert_eq!(scene.find_bone(rig, "head"), None, "the slot is missing");
    let before = scene.world.transform(head).unwrap().position;
    scene.build_skin_palettes();
    assert_eq!(scene.world.transform(head).unwrap().position, before);
}
