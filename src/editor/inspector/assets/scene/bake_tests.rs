//! A background bake belongs to the scene it started on (#1374).

use super::{is_open_scene, poll, start};
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
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.clone()));
    let mut scene = Scene::new();
    let mut console = ConsoleLogs::new();
    start(&mut editor, &mut scene, &mut console);
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

/// A bake that finishes while the scene plays waits for Stop: Play can start on a
/// frame the inspector is still drawn (#1374).
#[test]
fn a_bake_waits_while_the_scene_plays() {
    let path = crate::test_temp::dir().join("bake_play.scene");
    let path = path.to_string_lossy().into_owned();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.clone()));
    let mut scene = Scene::new();
    let mut console = ConsoleLogs::new();
    start(&mut editor, &mut scene, &mut console);
    scene.set_playing(true);
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    let ctx = egui::Context::default();
    for _ in 0..50 {
        poll(&mut editor, &ctx, &mut scene, &mut console, &nav);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(editor.dev.lightmap_bake.is_some(), "held, not applied");
    assert!(!std::path::Path::new(&path).exists(), "nothing was written");
}

/// Unity bakes the open scene only: another scene asset's card is not it, and the
/// bake writes the open scene's file, never the card's (#1394).
#[test]
fn only_the_open_scenes_card_bakes_and_the_bake_writes_the_open_file() {
    let dir = crate::test_temp::dir();
    let open = dir.join("bake_open.scene").to_string_lossy().into_owned();
    let other = dir.join("bake_other.scene").to_string_lossy().into_owned();
    std::fs::write(&open, "{}").unwrap();
    std::fs::write(&other, "{}").unwrap();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(open.clone()));
    assert!(is_open_scene(&editor, &open));
    assert!(!is_open_scene(&editor, &other), "another scene is not open");

    let mut scene = Scene::new();
    let mut console = ConsoleLogs::new();
    start(&mut editor, &mut scene, &mut console);
    let active = editor.dev.lightmap_bake.as_ref().expect("started");
    assert_eq!(active.job.scene_path(), open);
    assert!(!std::fs::read_to_string(&other)
        .unwrap()
        .contains("lightmap"));
}

/// The same file spelled relative and absolute is the open scene.
#[test]
fn a_relative_and_an_absolute_spelling_are_the_same_open_scene() {
    let dir = crate::test_temp::dir();
    let abs = dir.join("bake_spelling.scene");
    std::fs::write(&abs, "{}").unwrap();
    let cwd = std::env::current_dir().unwrap();
    let relative = pathdiff(&abs, &cwd);
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(abs.to_string_lossy().into_owned()));
    assert!(is_open_scene(&editor, &relative), "{relative}");
}

/// `abs` as a path relative to `from`, spelled with `..` where it must climb.
fn pathdiff(abs: &std::path::Path, from: &std::path::Path) -> String {
    let up = from.components().count();
    let mut out = std::path::PathBuf::new();
    for _ in 0..up {
        out.push("..");
    }
    out.join(abs.strip_prefix("/").unwrap_or(abs))
        .to_string_lossy()
        .into_owned()
}
