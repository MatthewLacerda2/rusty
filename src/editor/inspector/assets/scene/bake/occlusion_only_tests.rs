//! The Bake Occlusion button (#1958): the PVS bakes on a worker, then lands on
//! the scene and beside its file, with no lightmap or probe step.

use super::{poll, start_occlusion, Step};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::occlusion::{occlusion_path, test_level::corridors};
use crate::scripting::ConsoleLogs;

#[test]
fn the_button_bakes_the_pvs_alone_and_writes_it() {
    let dir = crate::test_temp::dir().join("editor_bake_occlusion");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = corridors();
    scene.save_to_file(&path).unwrap();
    let scene_text = std::fs::read(&path).unwrap();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.clone()));
    let mut console = ConsoleLogs::default();
    start_occlusion(&mut editor, &mut scene, &mut console);
    let active = editor.dev.lightmap_bake.as_ref().expect("started");
    assert!(matches!(active.step, Step::OcclusionOnly { .. }));
    assert!(scene.lighting_bake_running, "a script's bake waits");

    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        poll(&mut editor, &ctx, &mut scene, &mut console, &nav);
        if editor.dev.lightmap_bake.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(editor.dev.lightmap_bake.is_none(), "the bake ended");
    assert!(!scene.lighting_bake_running && !editor.is_dirty);
    assert!(scene.occlusion.is_some() && occlusion_path(&path).is_file());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        scene_text,
        "the scene file untouched"
    );
    let said = console
        .messages
        .iter()
        .any(|(m, _)| m.starts_with("Baked occlusion"));
    assert!(said, "{:?}", console.messages);
}
