//! Tests for linked prefab instances: live link, overrides, propagation (#216).

use super::*;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{
    apply_scene_data, load_and_instantiate_linked, save_prefab, to_scene_data, MaterialAsset,
    MaterialComponent,
};

/// Author a 2-entity prefab (Box root + Sphere child, root carrying a "Stone"
/// material) and save it to a unique temp `.prefab`. Returns its path.
fn write_prefab(tag: &str) -> String {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, "Child", Some(Primitive::Sphere));
    scene.set_parent(child, Some(root)).unwrap();
    scene
        .materials
        .insert("Stone".into(), MaterialAsset::default());
    scene.get_entity_mut(root).unwrap().material = Some(MaterialComponent {
        material: "Stone".into(),
    });
    let path = std::env::temp_dir()
        .join(format!("rusty_216_{tag}.prefab"))
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, root, &path).unwrap();
    path
}

/// Re-author the source prefab with the child renamed, to prove propagation pulls a
/// source edit into non-overridden fields.
fn rewrite_prefab_child_name(path: &str, new_child: &str) {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, new_child, Some(Primitive::Sphere));
    scene.set_parent(child, Some(root)).unwrap();
    save_prefab(&scene, root, path).unwrap();
}

#[test]
fn linked_instance_stamps_link_on_every_entity() {
    let path = write_prefab("stamp");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();

    let r = scene.get_entity(root).unwrap();
    let link = r.prefab_link.as_ref().expect("root is linked");
    assert_eq!(link.source, path);
    assert_eq!(link.local_id, 0, "root maps to source local id 0");
    let child = r.children[0];
    drop(r);
    let c = scene.get_entity(child).unwrap();
    let clink = c.prefab_link.as_ref().expect("child is linked too");
    assert_eq!(clink.source, path);
    assert_eq!(clink.local_id, 1, "child maps to source local id 1");
}

#[test]
fn override_survives_propagation() {
    let path = write_prefab("override_survives");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();

    // Override the root's position, then record it.
    scene.get_entity_mut(root).unwrap().transform.position.x = 5.0;
    record_instance_overrides(&mut scene, root).unwrap();
    assert!(!list_instance_overrides(&scene, root).unwrap().is_empty());

    // Reimport (propagation): the override is preserved.
    reimport_instance(&mut scene, root).unwrap();
    assert_eq!(scene.get_entity(root).unwrap().transform.position.x, 5.0);
}

#[test]
fn non_overridden_field_updates_on_source_edit() {
    let path = write_prefab("source_edit");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let child = scene.get_entity(root).unwrap().children[0];
    assert_eq!(scene.get_entity(child).unwrap().name, "Child");

    // Edit the SOURCE on disk, then reimport — the un-overridden child name updates.
    rewrite_prefab_child_name(&path, "Renamed");
    reimport_instance(&mut scene, root).unwrap();
    assert_eq!(scene.get_entity(child).unwrap().name, "Renamed");
}

#[test]
fn override_wins_over_a_conflicting_source_edit() {
    let path = write_prefab("override_wins");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let child = scene.get_entity(root).unwrap().children[0];

    // Override the child's name, record it, THEN change the same field in the source.
    scene.get_entity_mut(child).unwrap().name = "Mine".into();
    record_instance_overrides(&mut scene, root).unwrap();
    rewrite_prefab_child_name(&path, "SourceWins");

    reimport_instance(&mut scene, root).unwrap();
    assert_eq!(
        scene.get_entity(child).unwrap().name,
        "Mine",
        "a recorded override beats a source edit to the same field"
    );
}

#[test]
fn prefab_link_roundtrips_through_save_load() {
    let path = write_prefab("roundtrip");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    scene.get_entity_mut(root).unwrap().transform.position.y = 8.0;
    record_instance_overrides(&mut scene, root).unwrap();

    // Save -> load (apply_scene_data also runs propagation). The link + override
    // survive the JSON round-trip and the value is intact.
    let data = to_scene_data(&scene);
    let json = serde_json::to_string(&data).unwrap();
    let reloaded: SceneData = serde_json::from_str(&json).unwrap();
    let mut scene2 = Scene::new();
    apply_scene_data(&mut scene2, reloaded);

    let r = scene2.get_entity(root).unwrap();
    let link = r.prefab_link.as_ref().expect("link survived save/load");
    assert_eq!(link.source, path);
    assert!(!link.overrides.is_empty(), "overrides survived save/load");
    assert_eq!(r.transform.position.y, 8.0);
}

#[test]
fn revert_drops_overrides_and_restores_source() {
    let path = write_prefab("revert");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    scene.get_entity_mut(root).unwrap().transform.position.x = 99.0;
    record_instance_overrides(&mut scene, root).unwrap();

    revert_instance_overrides(&mut scene, root).unwrap();
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    assert_eq!(
        scene.get_entity(root).unwrap().transform.position.x, 0.0,
        "reverted to the source's value"
    );
}

#[test]
fn override_verbs_reject_a_non_instance() {
    let mut scene = Scene::new();
    let plain = create_entity(&mut scene, "Plain", None);
    assert!(record_instance_overrides(&mut scene, plain).is_err());
    assert!(reimport_instance(&mut scene, plain).is_err());
    assert!(list_instance_overrides(&scene, plain).is_err());
}
