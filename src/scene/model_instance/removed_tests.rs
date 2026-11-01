//! A part the user took out of a model instance stays out (#1665, Unity's
//! *Removed GameObject* override): destroyed, it is recorded as removed on the
//! instance and never stamped back by a load or a re-import; moved out of the
//! instance, it is unpacked (its link dropped) and recorded the same way. A
//! revert brings it back.

use glam::Vec3;

use super::tests::{child, spawn_file, KIT};
use crate::scene::asset_instance::set_model_scale_factor;
use crate::scene::authoring::revert_instance_field;
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::{list_instance_overrides, revert_instance_overrides, Scene};

/// How many children of `parent` are named `Wall`.
fn walls(scene: &Scene, parent: u32) -> usize {
    let wall = |id: &u32| scene.world.name(*id).is_some_and(|n| *n == "Wall");
    scene
        .world
        .children(parent)
        .iter()
        .filter(|id| wall(id))
        .count()
}

/// Spawn the kit as `file` and destroy its Wall. Returns the scene, the root,
/// the `Kit` entity and the model's path.
fn spawn_and_destroy(file: &str) -> (Scene, u32, u32, String) {
    let (mut scene, root) = spawn_file(file, KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let kit = child(&scene, root, "Kit");
    scene.destroy(child(&scene, kit, "Wall"));
    (scene, root, kit, path)
}

fn reload(scene: &Scene) -> Scene {
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, to_scene_data(scene));
    loaded
}

#[test]
fn a_destroyed_part_stays_removed_through_a_load() {
    let (scene, root, kit, _) = spawn_and_destroy("rusty_1665_del_load.gltf");
    let loaded = reload(&scene);
    assert_eq!(walls(&loaded, kit), 0, "not stamped back");
    let listed = list_instance_overrides(&loaded, root).unwrap();
    assert_eq!(listed, vec!["0:-part/Kit/Wall".to_string()]);
}

#[test]
fn a_destroyed_part_stays_removed_through_a_reimport() {
    let (mut scene, _, kit, path) = spawn_and_destroy("rusty_1665_del_scale.gltf");
    assert!(set_model_scale_factor(&mut scene, &path, 0.01).unwrap() > 0);
    assert_eq!(walls(&scene, kit), 0);
}

#[test]
fn reverting_a_removed_part_brings_it_back() {
    let (mut scene, root, kit, _) = spawn_and_destroy("rusty_1665_del_revert.gltf");
    assert!(revert_instance_field(&mut scene, root, "-part/Kit/Wall").unwrap());
    assert_eq!(walls(&scene, kit), 1, "the one key's revert");
    scene.destroy(child(&scene, kit, "Wall"));
    revert_instance_overrides(&mut scene, root).unwrap();
    assert_eq!(walls(&scene, kit), 1, "the whole instance's revert");
    let wall = child(&scene, kit, "Wall");
    assert!(scene
        .world
        .prefab_link(wall)
        .is_some_and(|l| l.local_id != 0));
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
}

#[test]
fn a_part_moved_out_of_its_instance_is_unpacked() {
    let (mut scene, root) = spawn_file("rusty_1665_drag_out.gltf", KIT, Vec3::ZERO);
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    scene.reparent(wall, None).unwrap();
    assert!(scene.world.prefab_link(wall).is_none(), "unlinked");
    let loaded = reload(&scene);
    assert_eq!(walls(&loaded, kit), 0, "not stamped a second time");
    assert_eq!(loaded.world.parent_id(wall), None, "the moved part is kept");
    assert!(loaded.world.mesh(wall).is_some());
    let listed = list_instance_overrides(&loaded, root).unwrap();
    assert_eq!(listed, vec!["0:-part/Kit/Wall".to_string()]);
}

#[test]
fn a_part_moved_inside_its_instance_stays_linked() {
    let (mut scene, root) = spawn_file("rusty_1665_move_in.gltf", KIT, Vec3::ZERO);
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    scene.reparent(wall, Some(root)).unwrap();
    assert!(scene.world.prefab_link(wall).is_some());
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert!(!listed.iter().any(|k| k.contains("-part/")), "{listed:?}");
}
