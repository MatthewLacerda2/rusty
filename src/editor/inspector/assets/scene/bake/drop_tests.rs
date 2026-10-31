//! A Generate Lighting dropped during its probe step says what reached the scene: the
//! lightmaps did, the probes did not (#1602). The probe step bakes from a snapshot, so
//! a probe removed midway, or the scene saved elsewhere, drops it with the probes'
//! old lighting kept.

use glam::Vec3;

use super::{poll, ActiveBake, Step};
use crate::dev::bake::generate_lighting::start_probes;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// An editor whose bake is in its probe step on a saved scene with two reflection
/// probes.
fn probing(name: &str) -> (EditorUi, Scene, String) {
    let dir = crate::test_temp::dir().join(format!("bake_drop_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    for x in [0.0, 4.0] {
        scene
            .reflection_probes
            .add(Vec3::new(x, 1.0, 0.0), Vec3::splat(2.0));
    }
    scene.lighting_settings.reflections.resolution = 16;
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.clone()));
    let job = start_probes(&mut scene, &path, None).unwrap();
    editor.dev.lightmap_bake = Some(ActiveBake {
        step: Step::Probes {
            job: Box::new(job),
            lightmaps: 0,
            path: path.clone(),
        },
        scene: scene.id(),
        path: Some(path.clone()),
    });
    (editor, scene, path)
}

fn poll_once(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    poll(editor, &egui::Context::default(), scene, console, &nav);
}

fn last_warning(console: &ConsoleLogs) -> &str {
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Warning, "{line}");
    line
}

#[test]
fn gpu_a_probe_removed_midway_drops_the_bake_and_says_what_was_applied() {
    let (mut editor, mut scene, _) = probing("removed");
    let mut console = ConsoleLogs::new();
    poll_once(&mut editor, &mut scene, &mut console);
    assert!(scene.lighting_bake_running, "a script's bake waits");
    scene.reflection_probes.remove(0);
    editor.is_dirty = false;
    poll_once(&mut editor, &mut scene, &mut console);
    assert!(editor.dev.lightmap_bake.is_none());
    assert!(!scene.lighting_bake_running);
    let line = last_warning(&console);
    assert!(line.contains("the probes changed"), "{line}");
    assert!(line.contains("lightmaps were applied"), "{line}");
    assert!(editor.is_dirty, "the applied lightmaps still need saving");
    assert!(scene.reflection_probes.probes[0].cubemap_path.is_empty());
}

/// Save As mid-step: the bake would write the old file as if it were current.
#[test]
fn gpu_saving_elsewhere_midway_drops_the_bake() {
    let (mut editor, mut scene, path) = probing("save_as");
    let mut console = ConsoleLogs::new();
    let elsewhere = path.replace("level.scene", "copy.scene");
    editor.set_current_scene_path(Some(elsewhere));
    poll_once(&mut editor, &mut scene, &mut console);
    assert!(editor.dev.lightmap_bake.is_none());
    let line = last_warning(&console);
    assert!(line.contains("saved elsewhere"), "{line}");
    assert!(line.contains("probes kept their old lighting"), "{line}");
    let dir = std::path::Path::new(&path).parent().unwrap();
    let left = std::fs::read_dir(dir).unwrap().flatten();
    assert!(left
        .into_iter()
        .all(|e| !e.file_name().to_string_lossy().ends_with(".baking")));
}
