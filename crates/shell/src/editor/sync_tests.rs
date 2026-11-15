//! The editor window's host flags hold from before its first frame: an editor
//! launched or attached while the screen is already locked has never drawn, and
//! the first frame drains commands before it would.

use super::mark_host;
use rusty_dev::session::Session;

/// A world marked as `EditorFrontend::new` marks it, nothing drawn, its scene saved.
fn never_drawn(tag: &str) -> Session {
    let s = Session::new("").expect("an empty session boots");
    mark_host(&mut s.world().resources.application.borrow_mut());
    let dir = rusty_test_support::temp::dir().join(format!("never_drawn_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    rusty_edit::save_to_file(&s.world().scene().borrow(), &path).unwrap();
    *s.world().script_manager().scene_path_cell().borrow_mut() = Some(path);
    s
}

/// The commands the channel drains on that first frame (through the same
/// evaluator) are accepted: a gizmo set, and a background bake.
#[test]
fn a_command_on_an_editor_that_never_drew_is_accepted() {
    let s = never_drawn("first_frame");
    let set = s.eval(r#"Editor.SetGizmo("NavMesh", true)"#);
    assert_eq!(set.as_deref(), Ok("true"));
    assert_eq!(s.eval("Lighting.GenerateAsync()").as_deref(), Ok("true"));
    assert!(s.world().scene().borrow().lighting_bake_requested);
}
