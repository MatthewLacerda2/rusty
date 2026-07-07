//! Tests for the editor prefab-instance override card's logic layer (#263): the
//! deferred-action dispatch and root resolution. The egui `draw` is left to manual
//! verification; what matters for correctness is that the buttons map onto the exact
//! same `scene::authoring` verbs the `Scene.*` API uses — proven here by driving
//! `dispatch` and comparing against the verbs directly.

use super::*;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{
    list_instance_overrides, load_and_instantiate_linked, record_instance_overrides, save_prefab,
    MaterialAsset, MaterialComponent, Scene,
};

/// Author a 2-entity prefab (Box root + Sphere child) to a unique temp `.prefab`.
fn write_prefab(tag: &str) -> String {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, "Child", Some(Primitive::Sphere));
    scene.set_parent(child, Some(root)).unwrap();
    scene
        .materials
        .insert("Stone".into(), MaterialAsset::default());
    scene.world.set_material(
        root,
        Some(MaterialComponent {
            material: "Stone".into(),
        }),
    );
    let path = std::env::temp_dir()
        .join(format!("rusty_263_{tag}.prefab"))
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, root, &path).unwrap();
    path
}

/// Stamp a fresh linked instance of a 2-entity prefab; returns (scene, root id).
fn linked(tag: &str) -> (Scene, u32) {
    let path = write_prefab(tag);
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    (scene, root)
}

#[test]
fn instance_root_walks_up_from_a_child() {
    let (scene, root) = linked("root_from_child");
    let child = scene.world.children(root)[0];
    // From either the root or the child, the resolved root is the same instance root.
    assert_eq!(instance_root(&scene, root), root);
    assert_eq!(instance_root(&scene, child), root);
}

#[test]
fn instance_root_of_a_plain_entity_is_itself() {
    let mut scene = Scene::new();
    let plain = create_entity(&mut scene, "Plain", None);
    assert_eq!(instance_root(&scene, plain), plain);
}

#[test]
fn record_action_records_overrides_like_the_api() {
    let (mut scene, root) = linked("record");
    scene.world.transform_mut(root).unwrap().position.x = 5.0;
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());

    assert!(dispatch(&mut scene, PrefabAction::Record(root)));
    // Same effect as Scene.RecordPrefabOverrides: the divergence is now recorded.
    assert!(!list_instance_overrides(&scene, root).unwrap().is_empty());
}

#[test]
fn apply_to_source_action_writes_prefab_and_clears_overrides() {
    let (mut scene, root) = linked("apply_to_source");
    scene.world.transform_mut(root).unwrap().position.x = 6.0;
    record_instance_overrides(&mut scene, root).unwrap();

    // Apply-to-source clears the instance overrides (it now matches the source) and the
    // value persists, baked into the .prefab. Same effect as Scene.ApplyPrefabToSource.
    assert!(dispatch(&mut scene, PrefabAction::ApplyToSource(root)));
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    assert_eq!(scene.world.transform(root).unwrap().position.x, 6.0);
}

#[test]
fn apply_field_to_source_action_clears_only_that_field() {
    let (mut scene, root) = linked("apply_field");
    {
        let mut t = scene.world.transform_mut(root).unwrap();
        t.position.x = 2.0;
        t.position.y = 3.0;
    }
    record_instance_overrides(&mut scene, root).unwrap();

    let action = PrefabAction::ApplyFieldToSource {
        entity: root,
        path: "/transform/position/0".to_string(),
    };
    assert!(dispatch(&mut scene, action));
    // X is now in the source (override cleared); Y stays an override.
    let remaining = list_instance_overrides(&scene, root).unwrap();
    assert_eq!(remaining, vec!["0:/transform/position/1".to_string()]);
    let t = scene.world.transform(root).unwrap();
    assert_eq!(t.position.x, 2.0);
    assert_eq!(t.position.y, 3.0);
}

#[test]
fn revert_action_drops_overrides_and_restores_source() {
    let (mut scene, root) = linked("revert");
    scene.world.transform_mut(root).unwrap().position.x = 9.0;
    record_instance_overrides(&mut scene, root).unwrap();

    assert!(dispatch(&mut scene, PrefabAction::Revert(root)));
    assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    assert_eq!(scene.world.transform(root).unwrap().position.x, 0.0);
}

#[test]
fn reimport_action_keeps_recorded_overrides() {
    let (mut scene, root) = linked("reimport");
    scene.world.transform_mut(root).unwrap().position.x = 7.0;
    record_instance_overrides(&mut scene, root).unwrap();

    assert!(dispatch(&mut scene, PrefabAction::Reimport(root)));
    assert_eq!(scene.world.transform(root).unwrap().position.x, 7.0);
}

#[test]
fn revert_field_drops_one_override_and_keeps_the_rest() {
    let (mut scene, root) = linked("revert_field");
    {
        let mut t = scene.world.transform_mut(root).unwrap();
        t.position.x = 3.0;
        t.position.y = 4.0;
    }
    record_instance_overrides(&mut scene, root).unwrap();

    // Revert only the X leaf: it returns to source (0.0), Y stays overridden (4.0).
    let action = PrefabAction::RevertField {
        root,
        entity: root,
        path: "/transform/position/0".to_string(),
    };
    assert!(dispatch(&mut scene, action));
    let t = scene.world.transform(root).unwrap();
    assert_eq!(t.position.x, 0.0, "reverted leaf tracks source");
    assert_eq!(t.position.y, 4.0, "other override preserved");
}

#[test]
fn revert_field_on_an_unknown_path_is_a_noop() {
    let (mut scene, root) = linked("revert_field_noop");
    scene.world.transform_mut(root).unwrap().position.x = 1.0;
    record_instance_overrides(&mut scene, root).unwrap();
    let before = list_instance_overrides(&scene, root).unwrap();

    let action = PrefabAction::RevertField {
        root,
        entity: root,
        path: "/not/a/real/path".to_string(),
    };
    assert!(
        !dispatch(&mut scene, action),
        "nothing removed -> no change"
    );
    assert_eq!(list_instance_overrides(&scene, root).unwrap(), before);
}

#[test]
fn actions_on_a_plain_entity_report_no_change() {
    let mut scene = Scene::new();
    let plain = create_entity(&mut scene, "Plain", None);
    // The verbs error on a non-instance, so dispatch reports "no change" (false).
    assert!(!dispatch(&mut scene, PrefabAction::Record(plain)));
    assert!(!dispatch(&mut scene, PrefabAction::Revert(plain)));
    assert!(!dispatch(&mut scene, PrefabAction::Reimport(plain)));
    assert!(!dispatch(&mut scene, PrefabAction::ApplyToSource(plain)));
}

#[test]
fn field_label_humanizes_a_json_pointer() {
    assert_eq!(
        field_label("/transform/position/0"),
        "transform / position / 0"
    );
    assert_eq!(field_label("/name"), "name");
    assert_eq!(field_label(""), "");
}
