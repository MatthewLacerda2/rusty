//! Tests for instant overrides against a changing source (#1527): a source applied to
//! during Play reaches the edit-mode instances on Stop, and one removed during Play
//! leaves its instances without a baseline, as a scene loaded without its source.

use super::tests::{linked, save_and_load};
use crate::scene::prefab::apply::apply_instance_to_source;
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::{list_instance_overrides, load_and_instantiate_linked};
use glam::Vec3;

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
fn a_source_removed_during_play_leaves_its_instance_without_a_baseline() {
    let (mut scene, root, child, path) = linked("play_removed");
    scene.world.transform_mut(child).unwrap().position.x = 4.0;
    let snapshot = SceneSnapshot::capture(&scene);
    std::fs::remove_file(&path).unwrap();
    snapshot.restore(&mut scene);

    for id in [root, child] {
        let link = scene.world.prefab_link(id).unwrap().clone();
        assert!(
            link.baseline.is_none(),
            "entity {id} keeps no stale baseline"
        );
    }
    assert_eq!(scene.world.transform(child).unwrap().position.x, 4.0);
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert_eq!(listed, ["1:/transform/position/0"], "the stored overrides");
    let back = save_and_load(&scene);
    assert_eq!(list_instance_overrides(&back, root).unwrap(), listed);
}
