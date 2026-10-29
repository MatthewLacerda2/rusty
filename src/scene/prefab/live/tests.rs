//! Tests for instant overrides (#1527): an instance edit is an override the moment it
//! is made, survives Play, a save/load and a reimport, and a source change still
//! reaches every leaf the instance did not edit.

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::{
    apply_scene_data, list_instance_overrides, load_and_instantiate_linked, reimport_instance,
    revert_instance_overrides, save_prefab, to_scene_data, Scene, SceneData,
};
use glam::Vec3;

/// A Box root at (0, 0.95, -15.5) with a Sphere child at (0, 1, 0), saved to a temp
/// `.prefab`; returns its path.
fn write_prefab(tag: &str, child_scale: f32) -> String {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, "Child", Some(Primitive::Sphere));
    scene.set_parent(child, Some(root)).unwrap();
    scene.world.transform_mut(root).unwrap().position = Vec3::new(0.0, 0.95, -15.5);
    let mut t = scene.world.transform_mut(child).unwrap();
    t.position = Vec3::new(0.0, 1.0, 0.0);
    t.scale = Vec3::splat(child_scale);
    drop(t);
    let path = crate::test_temp::dir()
        .join(format!("rusty_1527_{tag}.prefab"))
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, root, &path).unwrap();
    path
}

/// A linked instance of a fresh prefab: (scene, root, child, source path).
pub(super) fn linked(tag: &str) -> (Scene, u32, u32, String) {
    let path = write_prefab(tag, 1.0);
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let child = scene.world.children(root)[0];
    (scene, root, child, path)
}

/// `scene` saved to JSON and loaded back, as a scene file is.
pub(super) fn save_and_load(scene: &Scene) -> Scene {
    let json = serde_json::to_string(&to_scene_data(scene)).unwrap();
    let mut back = Scene::new();
    apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
    back
}

#[test]
fn a_moved_root_keeps_its_place_through_play() {
    let (mut scene, root, _, _) = linked("play_root");
    scene.world.transform_mut(root).unwrap().position = Vec3::new(5.0, 3.5, -20.0);
    let snapshot = SceneSnapshot::capture(&scene);
    scene.world.transform_mut(root).unwrap().position = Vec3::new(5.0, 3.38, -20.0);
    snapshot.restore(&mut scene);
    let pos = scene.world.transform(root).unwrap().position;
    assert_eq!(pos, Vec3::new(5.0, 3.5, -20.0), "Stop put the edit back");
    // The root's placement is the instance's own, never an override.
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
}

#[test]
fn the_root_placement_survives_reimport_revert_and_reload() {
    let (mut scene, root, _, _) = linked("root_place");
    scene.world.transform_mut(root).unwrap().position = Vec3::new(5.0, 3.5, -20.0);
    reimport_instance(&mut scene, root).unwrap();
    revert_instance_overrides(&mut scene, root).unwrap();
    let back = save_and_load(&scene);
    let pos = back.world.transform(root).unwrap().position;
    assert_eq!(pos, Vec3::new(5.0, 3.5, -20.0));
}

#[test]
fn a_child_edit_is_an_override_at_once_and_survives_play_and_reload() {
    let (mut scene, root, child, _) = linked("child_edit");
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    scene.world.transform_mut(child).unwrap().position.x = 4.0;
    scene.world.set_name(child, "Edited".into());
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert_eq!(listed, ["1:/name", "1:/transform/position/0"]);

    let snapshot = SceneSnapshot::capture(&scene);
    scene.world.transform_mut(child).unwrap().position.x = 40.0;
    snapshot.restore(&mut scene);
    let back = save_and_load(&scene);
    assert_eq!(back.world.transform(child).unwrap().position.x, 4.0);
    assert_eq!(*back.world.name(child).unwrap(), "Edited");
    assert_eq!(list_instance_overrides(&back, root).unwrap(), listed);
}

#[test]
fn a_source_change_reaches_only_the_leaves_the_instance_did_not_edit() {
    let (mut scene, root, child, path) = linked("source_change");
    scene.world.transform_mut(child).unwrap().position.x = 4.0;
    // The source changes underneath: the child's scale and its x both move.
    let mut edited = Scene::new();
    let src = load_and_instantiate_linked(&mut edited, &path, None).unwrap();
    let src_child = edited.world.children(src)[0];
    let mut t = edited.world.transform_mut(src_child).unwrap();
    t.scale = Vec3::splat(2.0);
    t.position.x = -7.0;
    drop(t);
    save_prefab(&edited, src, &path).unwrap();

    reimport_instance(&mut scene, root).unwrap();
    let t = scene.world.transform(child).unwrap().clone();
    assert_eq!(
        t.scale,
        Vec3::splat(2.0),
        "an unedited leaf tracks the source"
    );
    assert_eq!(t.position.x, 4.0, "the edited leaf stays the instance's");
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert_eq!(listed, ["1:/transform/position/0"], "scale is no override");
}

#[test]
fn a_fresh_or_reloaded_instance_has_no_overrides() {
    let (scene, root, _, _) = linked("pristine");
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    let mut back = save_and_load(&scene);
    assert!(list_instance_overrides(&back, root).unwrap().is_empty());
    reimport_instance(&mut back, root).unwrap();
    assert!(list_instance_overrides(&back, root).unwrap().is_empty());
}
