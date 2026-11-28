//! The follow with two scene files loaded: each file is watched, and a change to one reloads
//! it alone, the other's unsaved edits kept.

use super::SceneFollow;
use rusty_core::fs::written::note_written;
use rusty_edit::load::prepare::{PreparedScene, SceneSidecars};
use rusty_scene::rehydrate::RehydratedScene;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

fn write(path: &str, names: &[&str]) {
    let mut scene = Scene::new();
    for name in names {
        scene.add_entity(name.to_string());
    }
    rusty_edit::save_to_file(&scene, path).unwrap();
}

fn names(scene: &Scene) -> Vec<String> {
    let ids = scene.entity_ids().into_iter();
    ids.map(|id| scene.world.name(id).unwrap().clone())
        .collect()
}

/// `base` loaded, `level` joined beside it.
fn two(base: &str, level: &str) -> Scene {
    write(base, &["Player"]);
    write(level, &["Door"]);
    let mut scene = Scene::new();
    rusty_edit::load_from_file(&mut scene, base).unwrap();
    let text = std::fs::read_to_string(level).unwrap();
    let data = rusty_scene::parse_scene_text(&text, level).unwrap();
    let document = RehydratedScene::new(data);
    let prepared = PreparedScene::new(level.to_string(), document, SceneSidecars::read(level));
    rusty_edit::scenes::join_additive(&mut scene, Some(base.to_string()), prepared);
    scene.record_disk_text_also(level, text);
    scene
}

#[test]
fn two_files_changed_between_frames_both_reload() {
    let dir = rusty_test_support::temp::dir();
    let file = |n: &str| dir.join(n).to_string_lossy().into_owned();
    let (base, level) = (file("base.scene"), file("level.scene"));
    let mut scene = two(&base, &level);
    let (mut follow, mut console, mut selected) = (SceneFollow::new(), ConsoleLogs::new(), None);
    write(&base, &["Player", "Gun"]);
    write(&level, &["Door", "Window"]);
    note_written(&base);
    note_written(&level);
    assert!(follow.apply(&mut scene, Some(&base), &mut selected, &mut console));
    assert!(follow.apply(&mut scene, Some(&base), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Player", "Gun", "Door", "Window"]);
    assert!(!follow.apply(&mut scene, Some(&base), &mut selected, &mut console));
}

#[test]
fn a_change_to_one_loaded_file_reloads_it_and_keeps_the_others_edits() {
    let dir = rusty_test_support::temp::dir();
    let file = |n: &str| dir.join(n).to_string_lossy().into_owned();
    let (base, level) = (file("base.scene"), file("level.scene"));
    let mut scene = two(&base, &level);
    let (mut follow, mut console, mut selected) = (SceneFollow::new(), ConsoleLogs::new(), None);
    scene.add_entity("Unsaved".to_string());
    write(&level, &["Door", "Window"]);
    note_written(&level);
    assert!(follow.apply(&mut scene, Some(&base), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Player", "Unsaved", "Door", "Window"]);
    assert!(
        rusty_edit::has_unsaved_edits(&scene, &base),
        "the base's edit stays unsaved"
    );
    assert!(!rusty_edit::has_unsaved_edits(&scene, &level));
    let paths = scene.loaded.paths(None);
    assert_eq!(paths, [Some(base), Some(level)]);
}
