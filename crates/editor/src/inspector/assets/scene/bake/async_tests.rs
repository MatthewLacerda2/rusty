//! `Lighting.GenerateAsync()` presses the button (#1999): the editor's next frame
//! starts the same job, `Lighting.IsRunning()` reads it running until it ends, and
//! `Lighting.GetBakeReport` reads what it wrote.

use super::poll;
use crate::EditorUi;
use rusty_dev::session::Session;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// A session with its empty scene saved, and an editor window open on that file
/// (the window marks itself the bake's host).
fn open_editor() -> (Session, EditorUi) {
    open_editor_in("generate_async_round_trip")
}

/// [`open_editor`], its scene in a folder of its own.
pub(super) fn open_editor_in(tag: &str) -> (Session, EditorUi) {
    let s = Session::new("").expect("an empty session boots");
    s.world()
        .resources
        .application
        .borrow_mut()
        .mark_bake_host();
    let dir = rusty_test_support::temp::dir().join(tag);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    s.world().scene().borrow().save_to_file(&path).unwrap();
    *s.world().script_manager().scene_path_cell().borrow_mut() = Some(path.clone());
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path));
    (s, editor)
}

/// One editor frame's bake drive.
pub(super) fn frame(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
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
    run_out(&mut editor, &scene, &mut console);
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "false");
    assert!(status(&s).starts_with("done"), "{}", status(&s));
    let report = s.eval("return Lighting.GetBakeReport('lightmaps') ~= nil");
    assert_eq!(report.unwrap(), "true", "{:?}", console.messages);
}

/// Drive the editor's frames until its bake ends.
pub(super) fn run_out(
    editor: &mut EditorUi,
    scene: &std::cell::RefCell<Scene>,
    console: &mut ConsoleLogs,
) {
    for _ in 0..4000 {
        frame(editor, &mut scene.borrow_mut(), console);
        if editor.dev.lightmap_bake.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("the bake never ended: {:?}", console.messages);
}

/// The status a script reads.
pub(super) fn status(s: &Session) -> String {
    let line =
        "local st = Lighting.GetBakeStatus(); return st.state .. ' ' .. tostring(st.message)";
    s.eval(line).unwrap()
}

/// A job that fails reads `failed` with its line, not the old report's success.
#[test]
fn gpu_a_failed_bake_reads_failed_with_its_reason() {
    let (s, mut editor) = open_editor_in("generate_async_fails");
    // The scene's folder is a file before the job starts (starting it touches no
    // file), so every write the job makes beside the scene fails, the lighting
    // included. Swapping it under a running job raced the job's preview writer,
    // which recreates the folder (#2049).
    let path = editor.current_scene_path().unwrap();
    let folder = std::path::Path::new(&path).parent().unwrap();
    std::fs::remove_dir_all(folder).unwrap();
    std::fs::write(folder, "not a folder").unwrap();
    s.eval("Lighting.GenerateAsync()").unwrap();
    let scene = s.world().scene().clone();
    let mut console = ConsoleLogs::new();
    frame(&mut editor, &mut scene.borrow_mut(), &mut console);
    assert!(status(&s).starts_with("running"), "{}", status(&s));
    run_out(&mut editor, &scene, &mut console);
    let st = status(&s);
    assert!(st.starts_with("failed Generate Lighting failed"), "{st}");
}
