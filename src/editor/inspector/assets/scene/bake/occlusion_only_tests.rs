//! The Bake Occlusion button (#1958): the PVS bakes on a worker, then lands on
//! the scene and beside its file, with no lightmap or probe step.

use super::{poll, start_occlusion, Step};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::occlusion::{occlusion_path, test_level::corridors};
use crate::scene::Scene;
use crate::script::scripting::{ConsoleLogs, LogLevel};

/// The corridors level saved under `tag`, open in an editor whose Bake Occlusion
/// was just pressed.
fn started(tag: &str) -> (EditorUi, Scene, String, ConsoleLogs) {
    let dir = crate::test_temp::dir().join(format!("editor_bake_occlusion_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = corridors();
    scene.save_to_file(&path).unwrap();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.clone()));
    let mut console = ConsoleLogs::default();
    start_occlusion(&mut editor, &mut scene, &mut console);
    let active = editor.dev.lightmap_bake.as_ref().expect("started");
    assert!(matches!(active.step, Step::OcclusionOnly { .. }));
    assert!(scene.lighting_bake_running, "a script's bake waits");
    (editor, scene, path, console)
}

/// Poll until the bake ends.
fn run(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        poll(editor, &ctx, scene, console, &nav);
        if editor.dev.lightmap_bake.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the bake never ended");
}

#[test]
fn the_button_bakes_the_pvs_alone_and_writes_it() {
    let (mut editor, mut scene, path, mut console) = started("writes");
    let scene_text = std::fs::read(&path).unwrap();
    run(&mut editor, &mut scene, &mut console);
    assert!(!scene.lighting_bake_running && !editor.is_dirty);
    assert!(scene.occlusion.is_some() && occlusion_path(&path).is_file());
    assert_eq!(std::fs::read(&path).unwrap(), scene_text, "scene untouched");
    let said = (console.messages.iter()).any(|(m, _)| m.starts_with("Baked occlusion"));
    assert!(said, "{:?}", console.messages);
}

/// The PVS applied but its file not written: the scene's next save writes it, so
/// the editor marks it unsaved.
#[test]
fn a_pvs_it_could_not_write_marks_the_scene_unsaved() {
    let (mut editor, mut scene, path, mut console) = started("unwritable");
    std::fs::create_dir_all(occlusion_path(&path)).unwrap(); // a write must fail
    run(&mut editor, &mut scene, &mut console);
    assert!(scene.occlusion.is_some(), "applied");
    assert!(editor.is_dirty, "the save that writes it is owed");
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Error);
    assert!(line.starts_with("Bake Occlusion failed"), "{line}");
}

#[test]
fn a_replaced_scene_drops_it_under_its_own_name() {
    let (mut editor, _, _, mut console) = started("replaced");
    run(&mut editor, &mut Scene::new(), &mut console);
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Warning);
    assert_eq!(line, "Bake Occlusion dropped: the open scene changed");
}
