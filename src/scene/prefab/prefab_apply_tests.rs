//! Tests for apply-to-source: writing a linked instance's overrides back into the
//! source `.prefab`, then clearing them on the instance (#268).

use super::*;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{
    list_instance_overrides, load_and_instantiate_linked, read_prefab_file,
    record_instance_overrides, save_prefab, Scene,
};

/// Author a 2-entity prefab (Box root + Sphere child) at a unique temp path.
fn write_prefab(tag: &str) -> String {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, "Child", Some(Primitive::Sphere));
    scene.set_parent(child, Some(root)).unwrap();
    let path = std::env::temp_dir()
        .join(format!("rusty_268_{tag}.prefab"))
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, root, &path).unwrap();
    path
}

/// Stamp a fresh linked instance of the 2-entity prefab; returns (scene, root id).
fn linked(path: &str) -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, path, None).unwrap();
    (scene, root)
}

#[test]
fn apply_writes_into_the_prefab_and_clears_the_override() {
    let path = write_prefab("whole");
    let (mut scene, root) = linked(&path);
    scene.world.transform_mut(root).unwrap().position.x = 5.0;
    record_instance_overrides(&mut scene, root).unwrap();
    assert!(!list_instance_overrides(&scene, root).unwrap().is_empty());

    let applied = apply_instance_to_source(&mut scene, root).unwrap();
    assert_eq!(applied, 1, "one entity had overrides");
    // The override is gone from the instance (it now matches source) ...
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    assert_eq!(scene.world.transform(root).unwrap().position.x, 5.0);
    // ... and the edit is baked into the .prefab on disk.
    let on_disk = read_prefab_file(&path).unwrap();
    assert_eq!(on_disk.entities[0].transform.position.x, 5.0);
}

#[test]
fn a_second_instance_reflects_the_change_after_reimport() {
    let path = write_prefab("second");
    // Instance A records + applies an edit to source.
    let (mut a, root_a) = linked(&path);
    a.world.transform_mut(root_a).unwrap().position.y = 9.0;
    record_instance_overrides(&mut a, root_a).unwrap();
    apply_instance_to_source(&mut a, root_a).unwrap();

    // A pristine instance B picks the new source value up on reimport.
    let (mut b, root_b) = linked(&path);
    assert_eq!(b.world.transform(root_b).unwrap().position.y, 9.0);
    reimport_instance(&mut b, root_b).unwrap();
    assert_eq!(b.world.transform(root_b).unwrap().position.y, 9.0);
}

#[test]
fn per_field_apply_touches_only_that_leaf() {
    let path = write_prefab("field");
    let (mut scene, root) = linked(&path);
    {
        let mut t = scene.world.transform_mut(root).unwrap();
        t.position.x = 2.0;
        t.position.z = 4.0;
    }
    record_instance_overrides(&mut scene, root).unwrap();

    apply_instance_field_to_source(&mut scene, root, "/transform/position/0").unwrap();
    // Only X went to source; Z is still an instance override.
    assert_eq!(
        list_instance_overrides(&scene, root).unwrap(),
        vec!["0:/transform/position/2".to_string()]
    );
    let on_disk = read_prefab_file(&path).unwrap();
    assert_eq!(on_disk.entities[0].transform.position.x, 2.0);
    assert_eq!(on_disk.entities[0].transform.position.z, 0.0, "Z untouched");
}

#[test]
fn apply_a_child_field_resolves_the_root() {
    let path = write_prefab("child");
    let (mut scene, root) = linked(&path);
    let child = scene.world.children(root)[0];
    scene.world.set_name(child, "Renamed".into());
    record_instance_overrides(&mut scene, root).unwrap();

    // Pass the CHILD id: the verb walks up to the root and applies the child's edit.
    apply_instance_field_to_source(&mut scene, child, "/name").unwrap();
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    let on_disk = read_prefab_file(&path).unwrap();
    assert_eq!(on_disk.entities[1].name, "Renamed");
}

#[test]
fn applied_change_round_trips_through_save_load() {
    use crate::scene::{apply_scene_data, to_scene_data, SceneData};
    let path = write_prefab("roundtrip");
    let (mut scene, root) = linked(&path);
    scene.world.transform_mut(root).unwrap().position.x = 3.0;
    record_instance_overrides(&mut scene, root).unwrap();
    apply_instance_to_source(&mut scene, root).unwrap();

    // Save -> load (load runs propagation against the now-updated source).
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let reloaded: SceneData = serde_json::from_str(&json).unwrap();
    let mut scene2 = Scene::new();
    apply_scene_data(&mut scene2, reloaded);
    assert_eq!(scene2.world.transform(root).unwrap().position.x, 3.0);
    assert!(list_instance_overrides(&scene2, root).unwrap().is_empty());
}

#[test]
fn apply_rejects_a_non_instance() {
    let mut scene = Scene::new();
    let plain = create_entity(&mut scene, "Plain", None);
    assert!(apply_instance_to_source(&mut scene, plain).is_err());
    assert!(apply_instance_field_to_source(&mut scene, plain, "/name").is_err());
}

#[test]
fn per_field_apply_on_a_missing_override_errors() {
    let path = write_prefab("missing");
    let (mut scene, root) = linked(&path);
    // No override recorded at this path -> error, nothing written.
    assert!(apply_instance_field_to_source(&mut scene, root, "/transform/position/0").is_err());
}
