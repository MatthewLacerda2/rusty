//! A linked instance's members and root (#991): bones are transparent, anything
//! else not of the instance ends the walk, and a malformed hierarchy cannot hang it.

use super::{instance_members, instance_root};
use crate::components::PrefabLink;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

fn link(scene: &mut Scene, id: u32, source: &str, local_id: u32) {
    let link = PrefabLink {
        source: source.into(),
        local_id,
        ..Default::default()
    };
    scene.world.set_prefab_link(id, Some(link));
}

/// A linked character (root, rig) whose rig carries a hand bone holding a gun of the
/// same instance, plus a stranger under the hand: `(scene, [root, rig, hand, gun, other])`.
fn character() -> (Scene, [u32; 5]) {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", None);
    let rig = create_entity(&mut scene, "Rig", Some(Primitive::Box));
    let hand = create_entity(&mut scene, "Hand", None);
    let gun = create_entity(&mut scene, "Gun", Some(Primitive::Box));
    let other = create_entity(&mut scene, "Other", None);
    scene.set_parent(rig, Some(root)).unwrap();
    scene.set_parent(hand, Some(rig)).unwrap();
    scene.set_parent(gun, Some(hand)).unwrap();
    scene.set_parent(other, Some(hand)).unwrap();
    scene.world.mesh_mut(rig).unwrap().skeleton.bones = vec![Some(hand)];
    for (local, id) in [root, rig, gun].into_iter().enumerate() {
        link(&mut scene, id, "hero.prefab", local as u32);
    }
    (scene, [root, rig, hand, gun, other])
}

#[test]
fn members_reach_through_a_bone_but_not_past_a_stranger() {
    let (mut scene, [root, rig, _hand, gun, other]) = character();
    let deep = create_entity(&mut scene, "Deep", None);
    scene.set_parent(deep, Some(other)).unwrap();
    link(&mut scene, deep, "hero.prefab", 9);
    assert_eq!(
        instance_members(&scene, root),
        Some(vec![root, rig, gun]),
        "the gun on the hand is a member; nothing below the unlinked stranger is"
    );
    assert_eq!(instance_members(&scene, other), None);
}

#[test]
fn the_root_is_found_through_a_bone() {
    let (scene, [root, rig, hand, gun, _other]) = character();
    assert_eq!(instance_root(&scene, gun), Some(root));
    assert_eq!(instance_root(&scene, rig), Some(root));
    assert_eq!(instance_root(&scene, hand), None, "a bone is not linked");
}

#[test]
fn a_different_source_ends_the_upward_walk() {
    let (mut scene, [_root, _rig, _hand, gun, _other]) = character();
    link(&mut scene, gun, "rifle.prefab", 0);
    assert_eq!(instance_root(&scene, gun), Some(gun));
}

#[test]
fn a_children_cycle_cannot_hang_the_member_walk() {
    let (mut scene, [root, rig, _hand, gun, _other]) = character();
    scene.world.add_child(gun, root);
    assert_eq!(instance_members(&scene, root), Some(vec![root, rig, gun]));
}
