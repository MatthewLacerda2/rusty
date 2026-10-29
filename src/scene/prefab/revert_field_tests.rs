//! Tests for the per-field revert (#998): one override dropped, the leaf re-tracks
//! the source, everything else stays — and a failure changes nothing.

use super::*;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{list_instance_overrides, load_and_instantiate_linked, save_prefab, Scene};

/// A linked instance of a fresh Box-root + Sphere-child prefab: (scene, root, path).
fn linked(tag: &str) -> (Scene, u32, String) {
    let mut author = Scene::new();
    let root = create_entity(&mut author, "Root", Some(Primitive::Box));
    let child = create_entity(&mut author, "Child", Some(Primitive::Sphere));
    author.set_parent(child, Some(root)).unwrap();
    let path = crate::test_temp::dir()
        .join(format!("rusty_998_{tag}.prefab"))
        .to_string_lossy()
        .into_owned();
    save_prefab(&author, root, &path).unwrap();
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    (scene, root, path)
}

#[test]
fn reverting_a_child_field_resolves_the_root_and_keeps_the_rest() {
    let (mut scene, root, _) = linked("child");
    let child = scene.world.children(root)[0];
    scene.world.set_name(child, "Renamed".into());
    scene.world.transform_mut(root).unwrap().position.x = 5.0;

    // Pass the CHILD id: the verb walks up to the root and reimports the instance.
    assert_eq!(revert_instance_field(&mut scene, child, "/name"), Ok(true));
    assert_eq!(scene.world.name(child).unwrap().as_str(), "Child");
    assert_eq!(scene.world.transform(root).unwrap().position.x, 5.0);
    assert_eq!(
        list_instance_overrides(&scene, root).unwrap(),
        vec!["0:/transform/position/0".to_string()]
    );
}

#[test]
fn reverting_a_field_with_no_override_is_false() {
    let (mut scene, root, _) = linked("absent");
    assert_eq!(revert_instance_field(&mut scene, root, "/name"), Ok(false));
}

#[test]
fn reverting_on_an_unlinked_entity_errors() {
    let mut scene = Scene::new();
    let plain = create_entity(&mut scene, "Plain", None);
    assert!(revert_instance_field(&mut scene, plain, "/name").is_err());
}

#[test]
fn a_missing_source_errors_and_keeps_the_override() {
    let (mut scene, root, path) = linked("gone");
    scene.world.transform_mut(root).unwrap().position.x = 5.0;
    std::fs::remove_file(&path).unwrap();

    assert!(revert_instance_field(&mut scene, root, "/transform/position/0").is_err());
    assert_eq!(
        list_instance_overrides(&scene, root).unwrap(),
        vec!["0:/transform/position/0".to_string()]
    );
    assert_eq!(scene.world.transform(root).unwrap().position.x, 5.0);
}
