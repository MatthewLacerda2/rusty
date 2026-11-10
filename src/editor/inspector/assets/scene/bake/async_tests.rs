//! `Lighting.GenerateAsync()` presses the button (#1999): the editor's next frame
//! starts the same job, `Lighting.IsRunning()` reads it running until it ends, and
//! `Lighting.GetBakeReport` reads what it wrote.

use super::poll;
use crate::dev::session::Session;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// A session with its empty scene saved, and an editor window open on that file
/// (the window marks itself the bake's host).
fn open_editor() -> (Session, EditorUi) {
    let s = Session::new("").expect("an empty session boots");
    s.world()
        .resources
        .application
        .borrow_mut()
        .mark_bake_host();
    let dir = crate::test_temp::dir().join("generate_async_round_trip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    s.world().scene().borrow().save_to_file(&path).unwrap();
    *s.world().script_manager().scene_path_cell().borrow_mut() = Some(path.clone());
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path));
    (s, editor)
}

/// One editor frame's bake drive.
fn frame(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    poll(editor, &egui::Context::default(), scene, console, &nav);
}

#[test]
fn gpu_a_request_starts_the_buttons_job_runs_and_reports() {
    let (s, mut editor) = open_editor();
    assert_eq!(s.eval("Lighting.GenerateAsync()").unwrap(), "true");
    let mut console = ConsoleLogs::new();
    let scene = s.world().scene().clone();
    frame(&mut editor, &mut scene.borrow_mut(), &mut console);
    assert!(editor.dev.lightmap_bake.is_some(), "{:?}", console.messages);
    assert!(!scene.borrow().lighting_bake_requested, "taken once");
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "true");
    for _ in 0..4000 {
        if editor.dev.lightmap_bake.is_none() {
            break;
        }
        frame(&mut editor, &mut scene.borrow_mut(), &mut console);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(editor.dev.lightmap_bake.is_none(), "{:?}", console.messages);
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "false");
    let report = s.eval("return Lighting.GetBakeReport('lightmaps') ~= nil");
    assert_eq!(report.unwrap(), "true", "{:?}", console.messages);
}

/// Play started between the call and the frame: the frame refuses it, in the
/// console, and nothing runs.
#[test]
fn a_request_the_frame_finds_in_play_is_refused_there() {
    let (s, mut editor) = open_editor();
    s.eval("Lighting.GenerateAsync()").unwrap();
    let scene = s.world().scene().clone();
    scene.borrow_mut().set_playing(true);
    let mut console = ConsoleLogs::new();
    frame(&mut editor, &mut scene.borrow_mut(), &mut console);
    assert!(editor.dev.lightmap_bake.is_none());
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "false");
    let (line, _) = console.messages.last().expect("said why");
    assert_eq!(line, crate::scene::LIGHTING_IN_PLAY);
}
