//! Tests for instant overrides against a changing source (#1527): a source applied to
//! during Play reaches the edit-mode instances on Stop, and an override the source can
//! no longer hold is kept, as Unity keeps an unused one.

use super::live_tests::{linked, save_and_load};
use crate::scene::prefab::apply::{apply_instance_to_source, revert_instance_field};
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::{list_instance_overrides, load_and_instantiate_linked, reimport_instance};
use glam::Vec3;
use serde_json::json;

#[test]
fn a_source_applied_to_during_play_reaches_the_other_instances_on_stop() {
    let (mut scene, a, a_child, path) = linked("play_apply");
    let b = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let b_child = scene.world.children(b)[0];
    scene.world.transform_mut(b_child).unwrap().position.x = 4.0;

    let snapshot = SceneSnapshot::capture(&scene);
    scene.world.transform_mut(a_child).unwrap().scale = Vec3::splat(3.0);
    apply_instance_to_source(&mut scene, a).unwrap();
    snapshot.restore(&mut scene);

    let t = scene.world.transform(b_child).unwrap().clone();
    assert_eq!(t.scale, Vec3::splat(3.0), "the source's change reached B");
    assert_eq!(t.position.x, 4.0, "B keeps its own edit");
    let listed = list_instance_overrides(&scene, b).unwrap();
    assert_eq!(listed, ["1:/transform/position/0"]);
    let a_scale = scene.world.transform(a_child).unwrap().scale;
    assert_eq!(a_scale, Vec3::splat(3.0), "A's edit-mode copy took it too");
    assert!(list_instance_overrides(&scene, a).unwrap().is_empty());
}

#[test]
fn an_override_the_source_cannot_hold_is_kept_until_reverted() {
    let (mut scene, root, child, _) = linked("unused");
    scene.world.transform_mut(child).unwrap().position.x = 4.0;
    // An override on a field the source no longer has (as one from an older source).
    let gone = "/no_such_component/value";
    let mut link = scene.world.prefab_link_mut(child).unwrap();
    link.overrides.insert(gone.into(), json!(7));
    drop(link);

    reimport_instance(&mut scene, root).unwrap();
    let expected = [format!("1:{gone}"), "1:/transform/position/0".into()];
    assert_eq!(list_instance_overrides(&scene, root).unwrap(), expected);
    let mut back = save_and_load(&scene);
    assert_eq!(list_instance_overrides(&back, root).unwrap(), expected);
    let stored = back.world.prefab_link(child).unwrap().overrides.clone();
    assert_eq!(stored[gone], json!(7), "the save wrote it");

    assert!(revert_instance_field(&mut back, child, gone).unwrap());
    let listed = list_instance_overrides(&back, root).unwrap();
    assert_eq!(listed, ["1:/transform/position/0"], "a revert drops it");
}
