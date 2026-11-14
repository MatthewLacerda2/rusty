//! A Generate Lighting dropped during its occlusion step (#1714) keeps the
//! lighting: the lightmaps and probes were written into the scene file when the
//! probes committed, before the PVS started baking, and only the PVS is lost.

use glam::Vec3;

use super::{poll, ActiveBake, Step};
use crate::dev::bake::generate_lighting::start_probes;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::occlusion::occlusion_path;
use crate::scene::lighting::occlusion::test_level::block;
use crate::scene::Scene;
use crate::script::scripting::{ConsoleLogs, LogLevel};

fn poll_once(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    poll(editor, &egui::Context::default(), scene, console, &nav);
}

/// A saved scene with a wall the PVS bakes and one reflection probe the probe
/// step bakes, and an editor in that probe step.
fn probing(path: &str) -> (EditorUi, Scene) {
    let mut scene = Scene::new();
    block(
        &mut scene,
        "Wall",
        Vec3::new(-4.0, 0.0, -0.1),
        Vec3::new(4.0, 3.0, 0.1),
    );
    let i = scene
        .reflection_probes
        .add(Vec3::new(0.0, 1.0, 2.0), Vec3::splat(2.0));
    scene.reflection_probes.edit(i, |p| p.manual = true);
    scene.lighting_settings.reflections.resolution = 16;
    scene.save_to_file(path).unwrap();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.to_string()));
    let job = start_probes(&mut scene, path, None).unwrap();
    let step = Step::Probes {
        job: Box::new(job),
        lightmaps: 0,
        path: path.to_string(),
    };
    let (id, open) = (scene.id(), Some(path.to_string()));
    editor.dev.lightmap_bake = Some(ActiveBake {
        step,
        scene: id,
        path: open,
    });
    (editor, scene)
}

fn occluding(editor: &EditorUi) -> bool {
    let active = editor.dev.lightmap_bake.as_ref();
    active.is_some_and(|b| matches!(b.step, Step::Occlusion { .. }))
}

#[test]
fn gpu_a_bake_dropped_while_the_pvs_bakes_keeps_the_written_lighting() {
    let dir = crate::test_temp::dir().join("bake_drop_occlusion");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let (mut editor, mut scene) = probing(&path);
    let mut console = ConsoleLogs::new();
    for _ in 0..50 {
        if occluding(&editor) {
            break;
        }
        poll_once(&mut editor, &mut scene, &mut console);
    }
    assert!(
        occluding(&editor),
        "the probes committed: {:?}",
        console.messages
    );
    let cube = scene.reflection_probes.probes[0].cubemap_path.clone();
    assert!(!cube.is_empty(), "the probe was baked");
    // The scene is replaced (an outside edit reloads it): the PVS step is dropped.
    poll_once(&mut editor, &mut Scene::new(), &mut console);
    assert!(editor.dev.lightmap_bake.is_none());
    let (line, level) = console.messages.last().unwrap();
    assert_eq!(*level, LogLevel::Warning, "{line}");
    let mut back = Scene::new();
    back.load_from_file(&path).unwrap();
    assert_eq!(
        back.reflection_probes.probes[0].cubemap_path, cube,
        "written"
    );
    assert!(!occlusion_path(&path).exists(), "only the PVS was lost");
}
