use super::Session;
use rusty_scene::Scene;
use rusty_script::scripting::LogLevel;

fn saved(tag: &str) -> String {
    let dir = rusty_test_support::temp::dir().join(format!("session_boot_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("boot.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    scene.add_entity("Booted".into());
    rusty_edit::save_to_file(&scene, &path).unwrap();
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
    let _ = s.eval("Scene.Save()");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not a scene");
}

#[test]
fn a_broken_sidecar_at_boot_is_a_warning_and_keeps_the_file() {
    let path = saved("sidecar");
    std::fs::write(rusty_scene::sidecar_path(&path), "{ broken").unwrap();
    let s = Session::new(&path).unwrap();
    assert_eq!(scene_path(&s), Some(path));
    assert!(has(&s, LogLevel::Warning) && !has(&s, LogLevel::Error));
}
