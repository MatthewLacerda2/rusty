//! What is not an outside change, and a file that does not load yet (#1340).

use super::SceneFollow;
use crate::core::fs::written::note_written;
use crate::scene::{save_lighting, Scene};
use crate::scripting::{ConsoleLogs, LogLevel};

/// `names` saved at `path` by "someone outside the editor", noted as the watch does.
fn write_outside(path: &str, names: &[&str]) {
    let mut scene = Scene::new();
    for name in names {
        scene.add_entity(name.to_string());
    }
    scene.save_to_file(path).unwrap();
    note_written(path);
}

/// The editor with `path` open, and the follow's first frame run.
fn open(tag: &str) -> (String, Scene, SceneFollow) {
    let dir = crate::test_temp::dir().join(format!("follow_echo_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    write_outside(&path, &["Kept", "Gone"]);
    let mut scene = Scene::new();
    scene.load_from_file(&path).unwrap();
    let mut follow = SceneFollow::new();
    assert!(!frame(&mut follow, &mut scene, &path).0);
    (path, scene, follow)
}

/// One edit-mode frame: whether it reloaded, and what it logged.
fn frame(follow: &mut SceneFollow, scene: &mut Scene, path: &str) -> (bool, ConsoleLogs) {
    let mut console = ConsoleLogs::new();
    let mut selected = scene.selected_entity_id;
    let reloaded = follow.apply(scene, Some(path), &mut selected, &mut console);
    (reloaded, console)
}

#[test]
fn a_selected_entity_is_not_an_unsaved_edit() {
    let (path, mut scene, mut follow) = open("selected");
    scene.selected_entity_id = scene.find_entity_by_name("Kept");
    write_outside(&path, &["Kept"]);
    let (reloaded, console) = frame(&mut follow, &mut scene, &path);
    assert!(reloaded);
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Info, "{line}");
}

/// A `touch`, a rescan or the same bytes rewritten: noted, but the text is the
/// engine's own, so the working copy (and its unsaved edits) stays.
#[test]
fn the_same_bytes_noted_again_are_no_change() {
    let (path, mut scene, mut follow) = open("touch");
    scene.add_entity("Unsaved".into());
    note_written(&path);
    let (reloaded, console) = frame(&mut follow, &mut scene, &path);
    assert!(!reloaded);
    assert!(console.messages.is_empty(), "{:?}", console.messages);
    assert!(scene.find_entity_by_name("Unsaved").is_some());
}

/// Generate Lighting rewrites the file while edits are unsaved (#832): no reload,
/// and the edits are still unsaved when a real outside change comes.
#[test]
fn a_lighting_save_is_not_an_outside_change() {
    let (path, mut scene, mut follow) = open("lighting");
    scene.add_entity("Unsaved".into());
    scene.lighting_settings.lightmaps.samples = 7;
    save_lighting(&scene, &path).unwrap();
    note_written(&path);
    assert!(!frame(&mut follow, &mut scene, &path).0);
    assert!(scene.find_entity_by_name("Unsaved").is_some());
    write_outside(&path, &["Kept"]);
    let (reloaded, console) = frame(&mut follow, &mut scene, &path);
    assert!(reloaded);
    assert_eq!(console.messages.last().unwrap().1, LogLevel::Warning);
}

/// A half-written file stays pending and loads once it parses, with no new note.
#[test]
fn a_file_that_does_not_parse_yet_is_read_again() {
    let (path, mut scene, mut follow) = open("partial");
    std::fs::write(&path, "{ \"entities\": [").unwrap();
    note_written(&path);
    let (reloaded, console) = frame(&mut follow, &mut scene, &path);
    assert!(!reloaded);
    assert_eq!(console.messages.len(), 1, "warned once");
    let (reloaded, console) = frame(&mut follow, &mut scene, &path);
    assert!(
        !reloaded && console.messages.is_empty(),
        "retried, not warned again"
    );
    let mut done = Scene::new();
    done.add_entity("Kept".into());
    done.save_to_file(&path).unwrap(); // finished writing; not noted again
    assert!(frame(&mut follow, &mut scene, &path).0);
    assert!(scene.find_entity_by_name("Gone").is_none());
}
