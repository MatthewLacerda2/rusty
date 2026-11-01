//! A part moved under another inside its model instance still belongs to the
//! instance by its own node (#1665): destroying or unpacking the part it hangs
//! under records it removed too, and a re-import that loses both the part it
//! hangs under and a part under it takes both, keeping it.

use glam::Vec3;

use super::removed_tests::reload;
use super::tests::{child, spawn_file};
use crate::scene::{list_instance_overrides, reimport_instance, Scene};

/// `Kit` with three empty parts.
const TREE: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [
    { "name": "Kit", "children": [ 1, 2, 3 ] },
    { "name": "Wall", "translation": [0.0, 0.0, 5.0] },
    { "name": "Post", "translation": [1.0, 0.0, 0.0] },
    { "name": "Door", "translation": [0.0, 1.0, 0.0] }
  ]
}"#;

/// Spawn `TREE` as `file` and move its Post under its Wall. Returns the scene,
/// the root, `Kit`, the Wall and the Post.
fn spawn_post_under_wall(file: &str) -> (Scene, u32, u32, u32, u32) {
    let (mut scene, root) = spawn_file(file, TREE, Vec3::ZERO);
    let kit = child(&scene, root, "Kit");
    let (wall, post) = (child(&scene, kit, "Wall"), child(&scene, kit, "Post"));
    scene.reparent(post, Some(wall)).unwrap();
    assert!(
        scene.world.prefab_link(post).is_some(),
        "moved inside: linked"
    );
    (scene, root, kit, wall, post)
}

/// How many entities in `scene` are named `name`.
fn named(scene: &Scene, name: &str) -> usize {
    let ids = scene.entity_ids().into_iter();
    ids.filter(|&id| scene.world.name(id).is_some_and(|n| *n == name))
        .count()
}

fn removed(scene: &Scene, root: u32) -> Vec<String> {
    list_instance_overrides(scene, root).unwrap()
}

const BOTH: [&str; 2] = ["0:-part/Kit/Post", "0:-part/Kit/Wall"];

#[test]
fn destroying_a_part_records_the_parts_moved_under_it() {
    let (mut scene, root, kit, wall, _) = spawn_post_under_wall("rusty_1665_mv_del.gltf");
    scene.destroy(wall);
    assert_eq!(removed(&scene, root), BOTH);
    let loaded = reload(&scene);
    assert_eq!(named(&loaded, "Post"), 0, "no pristine Post stamped back");
    assert_eq!(named(&loaded, "Wall"), 0);
    assert_eq!(loaded.world.children(kit).len(), 1, "only the Door");
}

#[test]
fn unpacking_a_part_unpacks_the_parts_moved_under_it() {
    let (mut scene, root, _, wall, post) = spawn_post_under_wall("rusty_1665_mv_out.gltf");
    scene.reparent(wall, None).unwrap();
    assert!(scene.world.prefab_link(wall).is_none());
    assert!(scene.world.prefab_link(post).is_none(), "left with it");
    assert_eq!(removed(&scene, root), BOTH);
    let loaded = reload(&scene);
    assert_eq!(
        named(&loaded, "Post"),
        1,
        "the unpacked one, no second copy"
    );
    assert!(loaded.world.prefab_link(post).is_none());
    assert_eq!(loaded.world.parent_id(post), Some(wall));
}

#[test]
fn a_play_destroy_records_no_removed_part() {
    let (mut scene, root) = spawn_file("rusty_1665_mv_play.gltf", TREE, Vec3::ZERO);
    let wall = child(&scene, child(&scene, root, "Kit"), "Wall");
    scene.set_playing(true);
    scene.destroy(wall);
    assert!(removed(&scene, root).is_empty(), "discarded at Stop anyway");
}

#[test]
fn a_gone_part_under_a_kept_part_under_a_gone_part_goes() {
    let (mut scene, root, kit, wall, post) = spawn_post_under_wall("rusty_1665_mv_gone.gltf");
    let door = child(&scene, kit, "Door");
    scene.reparent(door, Some(post)).unwrap();
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let renamed = TREE
        .replace(r#""name": "Wall""#, r#""name": "Panel""#)
        .replace(r#""name": "Door""#, r#""name": "Hatch""#);
    std::fs::write(&path, renamed).unwrap();
    reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.world.contains(wall) && !scene.world.contains(door));
    assert_eq!(scene.world.parent_id(post), Some(kit), "re-homed, kept");
    assert!(
        scene.world.children(post).is_empty(),
        "{:?}",
        scene.world.children(post)
    );
    assert!(removed(&scene, root).is_empty());
}
