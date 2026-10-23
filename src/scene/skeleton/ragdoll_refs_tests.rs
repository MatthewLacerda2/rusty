//! A ragdoll's saved bone joint (`BoneBinding::bodies`) follows its prefab like any
//! other id leaf (#1264): each stamp's bone joins the bone or prop of its own copy.

use super::fixture::{connected, hero, joint_to};
use crate::scene::prefab::{extract_prefab, instantiate_prefab, PrefabData};
use crate::scene::Scene;

/// `Group { A (skinned), B (skinned), Post }`, A's `Joint1` jointed to what `to`
/// picks of (B, Post): the authoring scene and the prefab extracted from it.
fn jointed(tag: &str, to: fn(&Scene, u32, u32) -> Option<u32>) -> (Scene, PrefabData) {
    let mut scene = Scene::new();
    let group = scene.add_entity("Group".into());
    let a = hero(&mut scene, &format!("{tag}_a"));
    let b = hero(&mut scene, &format!("{tag}_b"));
    let post = scene.add_entity("Post".into());
    for id in [a, b, post] {
        scene.set_parent(id, Some(group)).unwrap();
    }
    let hand = scene.find_bone(a, "Joint1").unwrap();
    let body = to(&scene, b, post);
    joint_to(&mut scene, hand, body);
    let prefab = extract_prefab(&scene, group).unwrap();
    (scene, prefab)
}

/// Stamp `prefab` into `scene`: (its A, its B, its Post).
fn stamp(scene: &mut Scene, prefab: &PrefabData) -> (u32, u32, u32) {
    let root = instantiate_prefab(scene, prefab, None);
    let kids = scene.world.children(root);
    (kids[0], kids[1], kids[2])
}

fn hand_of(scene: &Scene, owner: u32) -> u32 {
    scene.find_bone(owner, "Joint1").unwrap()
}

#[test]
fn each_stamped_ragdoll_joins_its_own_copys_bone() {
    let (mut author, prefab) = jointed("ragdoll_pair", |s, b, _| s.find_bone(b, "Joint1"));
    let mut fresh = Scene::new();
    for scene in [&mut author, &mut fresh] {
        let first = stamp(scene, &prefab);
        let second = stamp(scene, &prefab);
        for (a, b, _) in [first, second] {
            let hand = hand_of(scene, a);
            assert_eq!(
                connected(scene, hand),
                Some(hand_of(scene, b)),
                "A′.hand → B′.hand"
            );
        }
    }
}

#[test]
fn a_stamped_bone_joins_its_own_copys_prop() {
    let (mut author, prefab) = jointed("ragdoll_prop", |_, _, post| Some(post));
    let mut fresh = Scene::new();
    for scene in [&mut author, &mut fresh] {
        let (a, _, post) = stamp(scene, &prefab);
        assert_eq!(connected(scene, hand_of(scene, a)), Some(post));
    }
}
