//! `Editor.SetGizmo` / `Editor.GetGizmo` (#1909): the Scene view's gizmo toggles
//! from Lua, through `Application`'s mailbox. The editor's own half (the window
//! taking the request at its frame) is `core::application::gizmos`' tests.

use super::super::session::Session;
use rusty_core::application::{Gizmo, SceneGizmos};

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

fn run(s: &mut Session, line: &str) -> String {
    s.command(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

/// A session marked as the editor window would mark it.
fn windowed() -> Session {
    let s = session();
    s.world()
        .resources
        .application
        .borrow_mut()
        .mark_gizmo_viewer();
    s
}

fn warned(s: &Session, text: &str) -> bool {
    let console = s.world().console().borrow();
    console.messages.iter().any(|(m, _)| m.contains(text))
}

/// The window takes a set: read back on the next line, and the editor's frame
/// applies it to its own set.
#[test]
fn set_gizmo_reads_back_and_reaches_the_editor() {
    let mut s = windowed();
    assert_eq!(run(&mut s, r#"Editor.GetGizmo("NavMesh")"#), "false");
    assert_eq!(run(&mut s, r#"Editor.SetGizmo("NavMesh", true)"#), "true");
    assert_eq!(run(&mut s, r#"Editor.GetGizmo("NavMesh")"#), "true");
    assert_eq!(
        run(&mut s, r#"Editor.SetGizmo("LightProbes", false)"#),
        "true"
    );

    let mut editor = SceneGizmos::default();
    let app = &s.world().resources.application;
    app.borrow_mut().sync_gizmos(&mut editor);
    assert!(editor.on(Gizmo::NavMesh));
    assert!(!editor.on(Gizmo::LightProbes));
    assert!(editor.on(Gizmo::Ui), "the others are left alone");
}

/// A host with no Scene view (the headless session) refuses a set, with a warning,
/// and keeps the defaults.
#[test]
fn a_host_without_a_scene_view_refuses() {
    let mut s = session();
    assert_eq!(run(&mut s, r#"Editor.SetGizmo("NavMesh", true)"#), "false");
    assert!(warned(&s, "only the editor window has a Scene view"));
    assert_eq!(run(&mut s, r#"Editor.GetGizmo("NavMesh")"#), "false");
}

/// An unknown name warns: `SetGizmo` answers `false`, `GetGizmo` `nil`.
#[test]
fn an_unknown_name_warns() {
    let mut s = windowed();
    assert_eq!(run(&mut s, r#"Editor.SetGizmo("navmesh", true)"#), "false");
    assert!(warned(&s, r#"no gizmo "navmesh""#));
    assert_eq!(run(&mut s, r#"Editor.GetGizmo("colliders")"#), "nil");
    assert!(warned(&s, r#"no gizmo "colliders""#));
}

/// `Editor.InspectReflectionProbe` (#2525) makes a probe the inspected one, whose
/// volume the Scene view draws; it holds until cleared, and an index past the
/// probes is an error a script reads, changing nothing.
#[test]
fn a_reflection_probe_is_inspected_until_cleared_and_a_stray_index_errors() {
    let mut s = session();
    run(&mut s, "Reflection.Add(0, 1, 0, 1, 1, 1)");
    run(&mut s, "Reflection.Add(5, 1, 0, 1, 1, 1)");
    assert_eq!(run(&mut s, "Editor.InspectedReflectionProbe()"), "nil");
    run(&mut s, "Editor.InspectReflectionProbe(1)");
    assert_eq!(run(&mut s, "Editor.InspectedReflectionProbe()"), "1");
    let caught = "local ok, e = pcall(Editor.InspectReflectionProbe, 2); return tostring(e)";
    let e = run(&mut s, caught);
    assert!(e.contains("no reflection probe 2: the scene has 2"), "{e}");
    assert_eq!(run(&mut s, "Editor.InspectedReflectionProbe()"), "1");
    run(&mut s, "Editor.ClearInspectedReflectionProbe()");
    assert_eq!(run(&mut s, "Editor.InspectedReflectionProbe()"), "nil");
    run(&mut s, "Editor.InspectReflectionProbe(0)");
    run(&mut s, "Reflection.Remove(0)");
    assert_eq!(
        run(&mut s, "Editor.InspectedReflectionProbe()"),
        "nil",
        "removed"
    );
}
