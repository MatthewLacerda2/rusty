//! The boot scene becomes the current scene file only when it replaced the World
//! (#1374): an unparseable one leaves Untitled, so an argument-less `Scene.Save()`
//! can't write an empty scene over a recoverable file.

use super::Session;
use crate::scene::Scene;
use crate::scripting::LogLevel;

/// A one-entity scene saved at a fresh path.
fn saved(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("session_boot_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("boot.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    scene.add_entity("Booted".into());
    scene.save_to_file(&path).unwrap();
    path
}

fn scene_path(s: &Session) -> Option<String> {
    s.world()
        .script_manager()
        .scene_path_cell()
        .borrow()
        .clone()
}

fn has(s: &Session, level: LogLevel) -> bool {
    let console = s.world().console().borrow();
    console.messages.iter().any(|(_, l)| *l == level)
}

#[test]
fn an_unparseable_boot_scene_leaves_untitled() {
    let path = saved("broken");
    std::fs::write(&path, "{ not a scene").unwrap();
    let s = Session::new(&path).unwrap();
    assert_eq!(scene_path(&s), None);
    assert!(has(&s, LogLevel::Error));
    let _ = s.eval("Scene.Save()"); // refused: no current file
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not a scene");
}

#[test]
fn a_broken_sidecar_at_boot_is_a_warning_and_keeps_the_file() {
    let path = saved("sidecar");
    std::fs::write(crate::scene::sidecar_path(&path), "{ broken").unwrap();
    let s = Session::new(&path).unwrap();
    assert_eq!(scene_path(&s), Some(path));
    assert!(has(&s, LogLevel::Warning) && !has(&s, LogLevel::Error));
}
