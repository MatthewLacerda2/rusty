//! src/scene/serialize_tests.rs — a loaded document's one-sided parent links
//! (PR #1774's review): a child naming a parent whose `children` lacks it must not
//! leave `activeInHierarchy` stale when the parent is switched off.

use crate::scene::{apply_scene_data, to_scene_data, Scene};

/// A scene file giving `child` the parent `group` while `group` lists no children.
fn one_sided() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let group = scene.add_entity("Group".to_string());
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(group)).unwrap();
    let mut data = to_scene_data(&scene);
    let at = |id| data.entities.iter().position(|e| e.id == id).unwrap();
    let parent = at(group);
    data.entities[parent].children.clear();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    (loaded, group, child)
}

#[test]
fn a_one_sided_link_in_a_file_still_hides_the_child_with_its_parent() {
    let (mut scene, group, child) = one_sided();
    assert_eq!(scene.world.one_sided_links(), vec![child], "found at load");
    assert!(scene.world.is_active_in_hierarchy(child));
    scene.world.set_active(group, false);
    assert!(!scene.world.is_active_in_hierarchy(child), "never stale");
}

#[test]
fn a_file_with_both_sides_has_no_one_sided_link() {
    let mut scene = Scene::new();
    let group = scene.add_entity("Group".to_string());
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(group)).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, to_scene_data(&scene));
    assert_eq!(loaded.world.one_sided_links(), Vec::<u32>::new());
}
