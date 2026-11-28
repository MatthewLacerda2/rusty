use super::Member;
use crate::Scene;

/// Edit mode: `base.scene` holds root `a`, `b.scene` root `b` with a child.
fn authoring() -> (Scene, u32, u32, u32, u32) {
    let mut scene = Scene::new();
    let a = scene.add_entity("A".into());
    let b = scene.add_entity("B".into());
    let child = scene.add_entity("Child".into());
    scene.set_parent(child, Some(b)).unwrap();
    let settings = scene.settings();
    let base = Some("base.scene".to_string());
    let handle = scene.join_loaded(base, "b.scene".into(), settings, &[b], Default::default());
    (scene, a, b, child, handle)
}

#[test]
fn move_to_takes_a_root_and_its_children_follow() {
    let (mut scene, a, b, child, handle) = authoring();
    assert_eq!(scene.member_of(a), Some(Member::Scene(Some(0))));
    scene.move_to_scene(a, handle).unwrap();
    assert_eq!(scene.member_of(a), Some(Member::Scene(Some(handle))));
    let why = scene.move_to_scene(child, 0).unwrap_err();
    assert!(why.contains("only a root"), "{why}");
    assert!(scene.move_to_scene(b, 9).is_err(), "9 is no loaded scene");
    assert_eq!(scene.members(Some(handle)), vec![a, b, child]);
    assert!(scene.members(Some(0)).is_empty());
}

#[test]
fn edit_mode_refuses_a_parent_in_another_scene_and_play_moves_it() {
    let (mut scene, a, b, _, handle) = authoring();
    let why = scene.set_parent(a, Some(b)).unwrap_err();
    assert!(why.contains("another loaded scene"), "{why}");
    assert_eq!(scene.world.parent_id(a), None, "nothing changed");
    scene.set_playing(true);
    scene.set_parent(a, Some(b)).unwrap();
    assert_eq!(scene.member_of(a), Some(Member::Scene(Some(handle))));
}

#[test]
fn an_edit_mode_unload_despawns_its_members_at_once() {
    let (mut scene, a, b, child, handle) = authoring();
    scene.unload_scene(handle).unwrap();
    assert!(scene.world.contains(a));
    assert!(!scene.world.contains(b) && !scene.world.contains(child));
    assert!(scene.unload_settled());
    assert!(scene.loaded.file(handle).is_none());
}

#[test]
fn an_empty_scene_is_added_with_the_worlds_layers_and_spawns_join_it_when_active() {
    let (mut scene, a, ..) = authoring();
    let empty = scene.add_empty_scene(None, "c.scene".into(), crate_empty(&scene));
    assert_eq!(scene.loaded.handle_of("c.scene"), Some(empty));
    let carried = scene
        .loaded
        .file(empty)
        .unwrap()
        .carried
        .as_deref()
        .unwrap();
    assert_eq!(carried.layers, scene.layers);
    scene.set_active_scene(empty);
    let spawned = scene.add_entity("Spawned".into());
    assert_eq!(scene.members(Some(empty)), vec![spawned]);
    assert_eq!(scene.member_of(a), Some(Member::Scene(Some(0))));
}

#[test]
fn undo_of_a_destroy_brings_the_roots_scene_back() {
    let (mut scene, _, b, child, handle) = authoring();
    scene.begin_edits();
    scene.destroy(b);
    scene.commit_edits(Some("Delete"));
    assert!(!scene.world.contains(child));
    scene.undo();
    assert_eq!(scene.member_of(b), Some(Member::Scene(Some(handle))));
    assert_eq!(scene.member_of(child), Some(Member::Scene(Some(handle))));
}

#[test]
fn undo_of_a_parenting_gives_the_root_its_scene_back() {
    let (mut scene, _, b, _, handle) = authoring();
    let other = scene.add_entity("Other".into());
    scene.move_to_scene(other, handle).unwrap();
    scene.begin_edits();
    scene.set_parent(other, Some(b)).unwrap();
    scene.commit_edits(Some("Parent"));
    assert_eq!(scene.loaded.tag_of(other), None, "a child carries no tag");
    scene.undo();
    assert_eq!(scene.world.parent_id(other), None);
    assert_eq!(scene.member_of(other), Some(Member::Scene(Some(handle))));
}

#[test]
fn a_new_entity_takes_its_parents_scene() {
    let (mut scene, _, b, _, handle) = authoring();
    let stamped = scene.add_entity("Stamped".into());
    scene.set_parent(stamped, Some(b)).unwrap();
    assert_eq!(scene.member_of(stamped), Some(Member::Scene(Some(handle))));
    assert!(!scene.members(Some(0)).contains(&stamped));
}

#[test]
fn undo_of_a_first_scene_roots_destroy_after_a_change_of_active_scene_keeps_its_scene() {
    let (mut scene, a, _, _, handle) = authoring();
    assert_eq!(
        scene.loaded.tag_of(a),
        None,
        "the first scene's roots start untagged"
    );
    scene.begin_edits();
    scene.destroy(a);
    scene.commit_edits(Some("Delete"));
    scene.set_active_scene(handle);
    scene.undo();
    assert_eq!(scene.member_of(a), Some(Member::Scene(Some(0))));
}

/// An empty scene's document, as the edit crate makes it (`rusty_edit::scenes::add_empty`).
fn crate_empty(scene: &Scene) -> crate::serialize::SceneData {
    let json = serde_json::json!({
        "entities": [],
        "next_entity_id": 1,
        "selected_entity_id": null,
        "layers": scene.layers,
    });
    serde_json::from_value(json).unwrap()
}
