//! src/scene/serialize_tests.rs — what applying a loaded document holds: a one-sided
//! parent link (PR #1774's review, #1805): a child naming a parent whose `children`
//! lacks it is listed there as it loads, so switching the parent off hides the child;
//! and the bake settings are held to their ranges (#1704).

use crate::scene::{apply_scene_data, to_scene_data, Scene};

/// A scene file giving `child` the parent `group` while `group` lists no children;
/// `late` saves the group after the child, so the child waits for it.
fn one_sided(late: bool) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let group = scene.add_entity("Group".to_string());
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(group)).unwrap();
    let mut data = to_scene_data(&scene);
    let at = |id| data.entities.iter().position(|e| e.id == id).unwrap();
    let parent = at(group);
    data.entities[parent].children.clear();
    if late {
        let doc = data.entities.remove(parent);
        data.entities.push(doc);
    }
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    (loaded, group, child)
}

#[test]
fn a_one_sided_link_in_a_file_is_listed_as_it_loads() {
    for late in [false, true] {
        let (mut scene, group, child) = one_sided(late);
        assert_eq!(scene.world.children(group), vec![child], "late: {late}");
        assert!(scene.world.is_active_in_hierarchy(child));
        scene.world.set_active(group, false);
        assert!(!scene.world.is_active_in_hierarchy(child), "never stale");
    }
}

#[test]
fn a_file_with_both_sides_loads_its_children_once() {
    let mut scene = Scene::new();
    let group = scene.add_entity("Group".to_string());
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(group)).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, to_scene_data(&scene));
    assert_eq!(loaded.world.children(group), vec![child]);
}

#[test]
fn a_hand_edited_max_lightmap_size_reads_back_as_the_page_it_bakes_at() {
    let mut data = to_scene_data(&Scene::new());
    data.lighting_settings.lightmaps.max_lightmap_size = 5000;
    data.lighting_settings.lightmaps.samples = 0;
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    let held = loaded.lighting_settings.lightmaps;
    assert_eq!(held.max_lightmap_size, 4096, "what GetSettings returns");
    assert_eq!(held.samples, 1, "the other knobs are held the same way");
}
