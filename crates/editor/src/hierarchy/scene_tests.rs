//! The hierarchy with several scene files loaded: a group per scene, and its header's verbs.

use super::flat::{Flattener, Group};
use super::scenes::{act, SceneAction};
use crate::EditorUi;
use rusty_scene::Scene;

/// `base.scene` holds `A`; `b.scene` holds `B` and its child.
fn two_scenes() -> (Scene, [u32; 3], u32) {
    let mut scene = Scene::new();
    let a = scene.add_entity("A".into());
    let b = scene.add_entity("B".into());
    let child = scene.add_entity("Child".into());
    scene.set_parent(child, Some(b)).unwrap();
    let settings = scene.settings();
    let base = Some("base.scene".to_string());
    let handle = scene.join_loaded(base, "b.scene".into(), settings, &[b], Default::default());
    (scene, [a, b, child], handle)
}

fn rows(scene: &Scene) -> Vec<(Option<Group>, u32, usize)> {
    let mut flat = Flattener::default();
    flat.flatten(&egui::Context::default(), scene);
    let ids = flat.rows.iter().map(|r| (r.group, r.id, r.depth));
    ids.map(|(g, id, depth)| (g, if g.is_some() { 0 } else { id }, depth))
        .collect()
}

#[test]
fn one_scene_has_no_group_and_two_have_one_each() {
    let mut one = Scene::new();
    let a = one.add_entity("A".into());
    assert_eq!(rows(&one), [(None, a, 0)]);
    let (scene, [a, b, child], handle) = two_scenes();
    let want = [
        (Some(Group::Scene(0)), 0, 0),
        (None, a, 1),
        (Some(Group::Scene(handle)), 0, 0),
        (None, b, 1),
        (None, child, 2),
    ];
    assert_eq!(rows(&scene), want);
}

#[test]
fn a_dont_destroy_on_load_root_gets_a_group_of_its_own() {
    let (mut scene, [a, ..], _) = two_scenes();
    scene.set_playing(true);
    scene.dont_destroy_on_load(a);
    let groups: Vec<Group> = rows(&scene).into_iter().filter_map(|r| r.0).collect();
    assert_eq!(groups.last(), Some(&Group::DontDestroyOnLoad));
    assert!(rows(&scene).contains(&(None, a, 1)));
}

#[test]
fn the_header_sets_the_active_scene_and_unloads_one() {
    let (mut scene, [a, b, _], handle) = two_scenes();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some("base.scene".into()));
    act(&mut editor, &mut scene, handle, SceneAction::SetActive);
    assert_eq!(editor.current_scene_path().as_deref(), Some("b.scene"));
    editor.selected_entity_id = Some(b);
    act(&mut editor, &mut scene, handle, SceneAction::Unload);
    assert!(!scene.world.contains(b) && scene.world.contains(a));
    assert_eq!(editor.current_scene_path().as_deref(), Some("base.scene"));
    assert_eq!(
        editor.selected_entity_id, None,
        "the selection went with it"
    );
}

#[test]
fn unloading_the_only_edited_scene_clears_the_unsaved_mark() {
    let dir = rusty_test_support::temp::dir();
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let (base, level) = (path("base.scene"), path("level.scene"));
    rusty_edit::save_to_file(&Scene::new(), &base).unwrap();
    let mut other = Scene::new();
    other.add_entity("Door".into());
    rusty_edit::save_to_file(&other, &level).unwrap();
    let mut scene = Scene::new();
    rusty_edit::load_from_file(&mut scene, &base).unwrap();
    rusty_edit::scenes::load_additive(&mut scene, Some(base.clone()), &level).unwrap();
    let door = scene.find_entity_by_name("Door").unwrap();
    scene.world.set_name(door, "Gate".into());
    let mut editor = EditorUi::new(Default::default());
    editor.is_dirty = true;
    let handle = scene.loaded.handle_of(&level).unwrap();
    act(&mut editor, &mut scene, handle, SceneAction::Unload);
    assert!(!editor.is_dirty, "the edit left with its scene");
}
