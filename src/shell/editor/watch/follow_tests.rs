use super::SceneFollow;
use crate::core::fs::written::note_written;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// A scene file holding `Root` with child `Gun`, plus each of `extra`, written by
/// "someone outside the editor" (saved, then noted as the watch notes it).
fn write_outside(path: &str, extra: &[&str]) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Root".to_string());
    let gun = scene.add_entity("Gun".to_string());
    scene.set_parent(gun, Some(root)).unwrap();
    for name in extra {
        scene.add_entity(name.to_string());
    }
    scene.save_to_file(path).unwrap();
    note_written(path);
}

/// The editor with `path` open, `Gun` selected, and the follow's first frame run.
fn open(tag: &str) -> (String, Scene, SceneFollow, Option<u32>) {
    let path = crate::test_temp::dir().join(format!("{tag}.scene"));
    let path = path.to_string_lossy().into_owned();
    write_outside(&path, &["Gone"]);
    let mut scene = Scene::new();
    scene.load_from_file(&path).unwrap();
    let mut follow = SceneFollow::new();
    let mut selected = scene.find_entity_by_name("Gun");
    let mut console = ConsoleLogs::new();
    assert!(!follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    (path, scene, follow, selected)
}

fn names(scene: &Scene) -> Vec<String> {
    let ids = scene.entity_ids().into_iter();
    ids.map(|id| scene.world.name(id).unwrap().clone())
        .collect()
}

#[test]
fn an_outside_change_reloads_and_keeps_the_selection() {
    let (path, mut scene, mut follow, mut selected) = open("clean");
    write_outside(&path, &[]);
    let mut console = ConsoleLogs::new();
    assert!(follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Root", "Gun"], "Gone was deleted on disk");
    assert_eq!(
        selected,
        scene.find_entity_by_name("Gun"),
        "kept by name path"
    );
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Info, "{line}");
    assert!(line.starts_with("Scene reloaded:"), "{line}");
}

#[test]
fn unsaved_edits_are_discarded_with_a_warning() {
    let (path, mut scene, mut follow, mut selected) = open("unsaved");
    scene.add_entity("Unsaved".to_string());
    write_outside(&path, &[]);
    let mut console = ConsoleLogs::new();
    assert!(follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Root", "Gun"], "the file wins");
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Warning, "{line}");
    assert!(line.contains("unsaved edits were discarded"), "{line}");
}

#[test]
fn the_editors_own_save_is_not_an_outside_change() {
    let (path, mut scene, mut follow, mut selected) = open("own_save");
    scene.add_entity("Saved".to_string());
    scene.save_to_file(&path).unwrap();
    note_written(&path); // the watch reports the editor's own write
    let mut console = ConsoleLogs::new();
    assert!(!follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert!(console.messages.is_empty(), "{:?}", console.messages);
    // The save is the new baseline: the next outside change finds nothing unsaved.
    write_outside(&path, &[]);
    assert!(follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert_eq!(console.messages.last().unwrap().1, LogLevel::Info);
}

#[test]
fn a_change_seen_in_play_applies_at_stop() {
    let (path, mut scene, mut follow, mut selected) = open("play");
    scene.set_playing(true);
    write_outside(&path, &[]);
    let mut console = ConsoleLogs::new();
    assert!(!follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Root", "Gun", "Gone"], "held through Play");
    scene.set_playing(false);
    assert!(follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Root", "Gun"]);
}

/// A broken lighting sidecar fails after the World was replaced: still a reload,
/// with the selection remapped, and the sidecar error is a warning (#1374).
#[test]
fn a_broken_sidecar_still_reloads_and_keeps_the_selection() {
    let (path, mut scene, mut follow, mut selected) = open("sidecar");
    write_outside(&path, &[]); // the save writes a sidecar too: break it after
    std::fs::write(crate::scene::sidecar_path(&path), "{ broken").unwrap();
    let mut console = ConsoleLogs::new();
    assert!(follow.apply(&mut scene, Some(&path), &mut selected, &mut console));
    assert_eq!(names(&scene), ["Root", "Gun"]);
    assert_eq!(selected, scene.find_entity_by_name("Gun"));
    let (sidecar, level) = &console.messages[0];
    assert_eq!(*level, LogLevel::Warning, "{sidecar}");
    assert!(sidecar.contains("lighting"), "{sidecar}");
}
