//! Component entity references (a Selectable's target graphic, #420) follow the
//! entity through prefab save, stamp and linked propagation.

use super::*;
use crate::components::SelectableComponent;
use crate::scene::authoring::create_entity;
use crate::scene::prefab::{extract_prefab, instantiate_prefab_linked, read_prefab_file};
use crate::scene::save_prefab;

/// A root Selectable targeting its child, with an Up target outside the subtree.
fn authored() -> (Scene, u32) {
    let mut scene = Scene::new();
    let outside = create_entity(&mut scene, "Outside", None);
    let root = create_entity(&mut scene, "Button", None);
    let child = create_entity(&mut scene, "Icon", None);
    scene.set_parent(child, Some(root)).expect("parent");
    let mut sel = SelectableComponent {
        target_graphic: Some(child),
        ..Default::default()
    };
    sel.select_on[0] = Some(outside);
    scene.world.set_selectable(root, Some(sel));
    (scene, root)
}

fn target(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.selectable(id).and_then(|s| s.target_graphic)
}

#[test]
fn extract_and_stamp_remap_references() {
    let (scene, root) = authored();
    let prefab = extract_prefab(&scene, root).expect("extracts");
    let sel = prefab.entities[0].selectable.clone().expect("selectable");
    assert_eq!(sel.target_graphic, Some(1), "the child's local id");
    assert_eq!(sel.select_on[0], None, "outside the subtree: dropped");
    let mut other = Scene::new();
    create_entity(&mut other, "Offset", None);
    let stamped = crate::scene::instantiate_prefab(&mut other, &prefab, None);
    let child = other.world.children(stamped)[0];
    assert_ne!(child, 1);
    assert_eq!(target(&other, stamped), Some(child));
}

#[test]
fn a_linked_instance_diffs_clean_and_propagates_in_its_own_ids() {
    let (scene, root) = authored();
    let path = std::env::temp_dir()
        .join("rusty_420_refs.prefab")
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, root, &path).expect("saves");
    let prefab = read_prefab_file(&path).expect("reads");
    let mut other = Scene::new();
    create_entity(&mut other, "Offset", None);
    let inst = instantiate_prefab_linked(&mut other, &prefab, None, &path);
    let child = other.world.children(inst)[0];
    record_instance_overrides(&mut other, inst).expect("records");
    let overrides = other
        .world
        .prefab_link(inst)
        .expect("linked")
        .overrides
        .clone();
    assert!(
        overrides.is_empty(),
        "an untouched reference is no override: {overrides:?}"
    );
    reimport_instance(&mut other, inst).expect("reimports");
    assert_eq!(target(&other, inst), Some(child));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn applying_a_changed_reference_writes_the_source_in_local_ids() {
    let (mut scene, root) = authored();
    let second = create_entity(&mut scene, "Second", None);
    scene.set_parent(second, Some(root)).expect("parent");
    let path = std::env::temp_dir()
        .join("rusty_420_refs_apply.prefab")
        .to_string_lossy()
        .into_owned();
    save_prefab(&scene, root, &path).expect("saves");
    let prefab = read_prefab_file(&path).expect("reads");
    let mut other = Scene::new();
    create_entity(&mut other, "Offset", None);
    let inst = instantiate_prefab_linked(&mut other, &prefab, None, &path);
    let second_inst = other.world.children(inst)[1];
    other
        .world
        .selectable_mut(inst)
        .expect("sel")
        .target_graphic = Some(second_inst);
    record_instance_overrides(&mut other, inst).expect("records");
    crate::scene::prefab::apply::apply_instance_to_source(&mut other, inst).expect("applies");
    let source = read_prefab_file(&path).expect("reads");
    let sel = source.entities[0].selectable.clone().expect("selectable");
    assert_eq!(sel.target_graphic, Some(2), "the second child's local id");
    assert_eq!(target(&other, inst), Some(second_inst));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_markers_world_anchor_target_follows_the_prefab() {
    use crate::components::{RectTransformComponent, WorldAnchor};
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Enemy", None);
    let bar = create_entity(&mut scene, "HealthBar", None);
    scene.set_parent(bar, Some(root)).expect("parent");
    let rt = RectTransformComponent {
        world_anchor: Some(WorldAnchor {
            target: Some(root),
            ..Default::default()
        }),
        ..Default::default()
    };
    scene.world.set_rect_transform(bar, Some(rt));
    let prefab = extract_prefab(&scene, root).expect("extracts");
    let mut other = Scene::new();
    create_entity(&mut other, "Offset", None);
    let stamped = crate::scene::instantiate_prefab(&mut other, &prefab, None);
    let bar = other.world.children(stamped)[0];
    let anchor = other
        .world
        .rect_transform(bar)
        .and_then(|r| r.world_anchor.clone());
    assert_eq!(anchor.and_then(|a| a.target), Some(stamped));
    assert!(Entity::is_ref_pointer(
        "/rect_transform/world_anchor/target"
    ));
}
