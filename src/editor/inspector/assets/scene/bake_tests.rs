//! A background bake belongs to the scene it started on (#1374).

use super::{poll, start};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// The follow reloads the same file after an outside edit: same path, new World.
/// The bake's lightmaps (keyed by the old entity ids) are dropped, not applied.
#[test]
fn a_bake_is_dropped_when_the_scene_is_reloaded_under_the_same_path() {
    let path = crate::test_temp::dir().join("bake_reload.scene");
    let path = path.to_string_lossy().into_owned();
    let mut editor = EditorUi::new();
    editor.set_current_scene_path(Some(path.clone()));
    let mut scene = Scene::new();
    let mut console = ConsoleLogs::new();
    start(&mut editor, &mut scene, &mut console, &path);
    assert!(editor.dev.lightmap_bake.is_some(), "{:?}", console.messages);

    scene.renew_id(); // what a reload does to the World's identity
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    let ctx = egui::Context::default();
    poll(&mut editor, &ctx, &mut scene, &mut console, &nav);
    assert!(editor.dev.lightmap_bake.is_none());
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Warning, "{line}");
    assert!(line.contains("dropped"), "{line}");
    assert!(!std::path::Path::new(&path).exists(), "nothing was written");
}
