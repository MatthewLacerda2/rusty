//! A model instance takes its model's node tree (#1665, Unity's model prefab): a
//! node the model lost takes its entity (and its overrides) with it, what the user
//! added under it moves up keeping its place in the world, and a node the model
//! gained is spawned under its parent node's entity, in the open scene and on load.

use glam::Vec3;

use super::tests::{child, spawn_file, KIT};
use crate::scene::asset_instance::set_model_scale_factor;
use crate::scene::authoring::{create_entity, revert_instance_field};
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::{list_instance_overrides, reimport_instance, Scene};

/// Spawn the kit as `file` and rename its `Wall` node to `Panel` in the file, so
/// the placed `Wall` is gone from the model. Returns the scene, the root, the
/// `Kit` entity, the Wall and the model's path.
fn spawn_then_rename(file: &str) -> (Scene, u32, u32, u32, String) {
    let (scene, root) = spawn_file(file, KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    let renamed = KIT.replace(r#""name": "Wall""#, r#""name": "Panel""#);
    std::fs::write(&path, renamed).unwrap();
    (scene, root, kit, wall, path)
}

#[test]
fn a_removed_node_takes_its_part_and_its_overrides() {
    let (mut scene, root, kit, wall, _) = spawn_then_rename("rusty_1665_removed.gltf");
    scene.world.transform_mut(wall).unwrap().scale = Vec3::splat(2.0);
    reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.world.contains(wall), "the gone part is destroyed");
    let panel = child(&scene, kit, "Panel");
    let link = scene.world.prefab_link(panel).unwrap();
    assert_eq!(link.node.as_deref(), Some("Kit/Panel"));
    assert_eq!(scene.world.transform(panel).unwrap().scale, Vec3::ONE);
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert!(listed.is_empty(), "the override went with it: {listed:?}");
}

#[test]
fn what_the_user_added_under_a_removed_part_moves_up_in_place() {
    let (mut scene, root, kit, wall, _) = spawn_then_rename("rusty_1665_orphan.gltf");
    let lamp = create_entity(&mut scene, "Lamp", None);
    scene.set_parent(lamp, Some(wall)).unwrap();
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(1.0, 2.0, 3.0);
    let before = scene.world.world_position(lamp);
    reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.world.contains(wall));
    assert_eq!(
        scene.world.parent_id(lamp),
        Some(kit),
        "nearest surviving ancestor"
    );
    let after = scene.world.world_position(lamp);
    assert!(after.abs_diff_eq(before, 1e-5), "{before} -> {after}");
}

#[test]
fn a_scale_factor_change_reshapes_the_open_instance_unrecorded() {
    let (mut scene, _, kit, wall, path) = spawn_then_rename("rusty_1665_scale.gltf");
    assert!(set_model_scale_factor(&mut scene, &path, 0.01).unwrap() > 0);
    assert!(!scene.world.contains(wall));
    let panel = child(&scene, kit, "Panel");
    let at = scene.world.transform(panel).unwrap().position;
    assert!(at.abs_diff_eq(Vec3::new(0.0, 0.0, 0.05), 1e-6), "{at}");
    assert_eq!(scene.undo(), None, "a re-stamp is no edit to undo");
}

#[test]
fn a_saved_scene_takes_the_new_tree_on_load() {
    let (scene, root, kit, wall, _) = spawn_then_rename("rusty_1665_load.gltf");
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, to_scene_data(&scene));
    assert!(!loaded.world.contains(wall));
    let panel = child(&loaded, kit, "Panel");
    assert!(loaded.world.prefab_link(panel).is_some());
    assert_eq!(loaded.world.children(root), vec![kit]);
}

#[test]
fn reverting_a_part_gone_since_the_last_reimport_says_so() {
    let (mut scene, root, _, wall, _) = spawn_then_rename("rusty_1665_revert.gltf");
    let err = revert_instance_field(&mut scene, wall, "/transform/scale/0").unwrap_err();
    assert!(err.contains("Kit/Wall") && err.contains("gone"), "{err}");
    assert!(
        scene.world.contains(wall),
        "a refused revert changes nothing"
    );
    reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.world.contains(wall));
}

#[test]
fn another_spelling_of_the_model_path_finds_its_instances() {
    let (mut scene, root) = spawn_file("rusty_1637_spelling.gltf", KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let (dir, file) = path.rsplit_once('/').unwrap();
    let other = format!("{dir}/./{file}");
    let wall = child(&scene, child(&scene, root, "Kit"), "Wall");
    assert!(set_model_scale_factor(&mut scene, &other, 0.01).unwrap() > 0);
    let at = scene.world.transform(wall).unwrap().position;
    assert!(at.abs_diff_eq(Vec3::new(0.0, 0.0, 0.05), 1e-6), "{at}");
}

#[test]
fn a_nested_instance_of_the_same_model_is_left_its_own() {
    let (mut scene, a) = spawn_file("rusty_1665_nested.gltf", KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(a).unwrap().source.clone();
    let asset = crate::asset::imported(std::path::Path::new(&path)).unwrap();
    let b = super::instantiate_model(&mut scene, &path, &asset, Vec3::ZERO);
    let a_wall = child(&scene, child(&scene, a, "Kit"), "Wall");
    scene.set_parent(b, Some(a_wall)).unwrap();
    let b_wall = child(&scene, child(&scene, b, "Kit"), "Wall");
    scene.world.transform_mut(b_wall).unwrap().scale = Vec3::splat(3.0);
    reimport_instance(&mut scene, a).unwrap();
    assert_eq!(scene.world.parent_id(b), Some(a_wall), "B is not A's part");
    assert_eq!(
        scene.world.transform(b_wall).unwrap().scale,
        Vec3::splat(3.0)
    );
    let listed = list_instance_overrides(&scene, a).unwrap();
    assert!(listed.is_empty(), "B's edit is not A's: {listed:?}");
    let listed = list_instance_overrides(&scene, b).unwrap();
    assert!(
        listed.contains(&"2:/transform/scale/0".to_string()),
        "{listed:?}"
    );
}
