use std::path::{Path, PathBuf};

use crate::{EditorUi, Paint};
use rusty_dev::bake::queue::{read, Read, Runner, State};
use rusty_edit::authoring::{primitive_mesh_component, Primitive};
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::bake_status::BakeStatus;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

fn floor(file: &Path) {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".into());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(floor, true);
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (1.0, 2, 1, 32);
    rusty_edit::save_to_file(&scene, &file.to_string_lossy()).unwrap();
}

/// Three saved scenes and an editor open on `start.scene`, its queue under `root`.
fn editor(tag: &str) -> (PathBuf, EditorUi, Scene) {
    let root = rusty_test_support::temp::dir().join(tag);
    std::fs::create_dir_all(&root).unwrap();
    for name in ["start", "a", "b"] {
        floor(&root.join(format!("{name}.scene")));
    }
    let open = root.join("start.scene").to_string_lossy().into_owned();
    let mut editor = EditorUi::new(Default::default());
    let mut scene = Scene::new();
    editor.open_scene(&mut scene, &mut ConsoleLogs::new(), &open, false);
    editor.dev.bake_queue.root = root.clone();
    (root, editor, scene)
}

fn ask(root: &Path, scene: &mut Scene, paths: &[&str]) {
    let paths = paths.iter().map(|p| p.to_string()).collect();
    drop(Runner::start(root, paths).unwrap());
    scene.lighting_queue_requested = true;
}

fn frames(editor: &mut EditorUi, scene: &mut Scene, until: impl Fn(&EditorUi) -> bool) {
    let (ctx, mut nav) = (egui::Context::default(), NavigationGraph::from_scene(scene));
    let mut console = ConsoleLogs::new();
    for _ in 0..4000 {
        editor.frame_work(&ctx, scene, &mut console, &mut nav, false, Paint::Undrawn);
        if until(editor) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("the queue never got there: {:?}", console.messages);
}

fn states(root: &Path) -> Vec<State> {
    let Read::Found(q) = read(root) else {
        panic!("no queue file")
    };
    q.scenes.iter().map(|e| e.state).collect()
}

#[test]
fn gpu_the_editor_bakes_each_scene_in_turn_then_reopens_the_first() {
    let (root, mut editor, mut scene) = editor("editor_queue_bakes");
    ask(&root, &mut scene, &["a.scene", "gone.scene", "b.scene"]);
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_some()
    });
    assert!(scene.bake_busy(), "Play is refused while the queue runs");
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_none()
    });
    assert_eq!(states(&root), [State::Done, State::Failed, State::Done]);
    assert!(root.join("a.scene.lightmaps").is_dir() && root.join("b.scene.lightmaps").is_dir());
    let open = editor.current_scene_path().unwrap();
    assert!(open.ends_with("start.scene"), "{open}");
    assert!(!scene.bake_busy() && scene.lighting_queue.is_none());
    let BakeStatus::Failed(line) = &scene.lighting_bake_status else {
        panic!("{:?}", scene.lighting_bake_status)
    };
    assert!(line.contains("2 of 3 scene(s) done, 1 failed"), "{line}");
}

#[test]
fn gpu_a_cancel_stops_the_queue_leaving_the_rest_pending() {
    let (root, mut editor, mut scene) = editor("editor_queue_cancel");
    ask(&root, &mut scene, &["a.scene", "b.scene"]);
    frames(&mut editor, &mut scene, |e| e.dev.lightmap_bake.is_some());
    editor.dev.lightmap_bake.as_ref().unwrap().cancel();
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_none()
    });
    assert_eq!(states(&root), [State::Pending, State::Pending]);
    assert!(matches!(
        scene.lighting_bake_status,
        BakeStatus::Cancelled(_)
    ));
    // Resume continues it from its first scene.
    scene.lighting_queue_requested = true;
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_some()
    });
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_none()
    });
    assert_eq!(states(&root), [State::Done, State::Done]);
}

#[test]
fn a_request_with_nothing_to_resume_reads_refused() {
    let (_root, mut editor, mut scene) = editor("editor_queue_none");
    scene.lighting_queue_requested = true;
    frames(&mut editor, &mut scene, |_| true);
    assert!(editor.dev.bake_queue.running.is_none());
    let BakeStatus::Refused(why) = &scene.lighting_bake_status else {
        panic!()
    };
    assert!(why.contains("no bake queue"), "{why}");
}

mod unsaved;
