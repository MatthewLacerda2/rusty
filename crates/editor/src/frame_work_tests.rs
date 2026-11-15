//! A frame the window does not draw still does the editor's work: a script's
//! `Lighting.GenerateAsync()` starts and runs to its end with no frame ever
//! painted (the screen locked), and an edit reaches the navmesh outside Play.

use crate::{EditorUi, Paint};
use glam::Vec3;
use rusty_navigation::test_support::{add_box, add_floor, move_to};
use rusty_navigation::{NavigationGraph, Rebake};
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// A floor with one static crate, baked, then the crate moved without a rebake.
fn moved_crate() -> (Scene, NavigationGraph) {
    let mut scene = Scene::new();
    add_floor(&mut scene, -8.0, 8.0, -8.0, 8.0);
    let crate_id = add_box(&mut scene, Vec3::new(-1.0, 0.0, -1.0), Vec3::ONE);
    let mut nav = NavigationGraph::from_scene(&scene);
    nav.bake(&scene);
    move_to(&mut scene, crate_id, Vec3::new(4.0, 0.5, 4.0));
    (scene, nav)
}

fn undrawn(scene: &mut Scene, nav: &mut NavigationGraph, playing: bool) {
    let mut editor = EditorUi::new(Default::default());
    let ctx = egui::Context::default();
    editor.frame_work(
        &ctx,
        scene,
        &mut ConsoleLogs::new(),
        nav,
        playing,
        Paint::Undrawn,
    );
}

#[test]
fn an_undrawn_edit_mode_frame_syncs_the_navmesh() {
    let (mut scene, mut nav) = moved_crate();
    undrawn(&mut scene, &mut nav, false);
    assert!(
        matches!(nav.sync(&scene), Rebake::Unchanged),
        "already synced"
    );
}

#[test]
fn an_undrawn_play_frame_leaves_the_navmesh_to_play_mode() {
    let (mut scene, mut nav) = moved_crate();
    undrawn(&mut scene, &mut nav, true);
    assert!(matches!(nav.sync(&scene), Rebake::Incremental(_)));
}

/// The locked screen: the request is made through the command evaluator (the
/// attach bridge's), and only undrawn frames follow, never `EditorUi::draw`.
#[cfg(feature = "dev")]
#[test]
fn gpu_a_requested_bake_runs_to_its_end_on_undrawn_frames() {
    use crate::inspector::assets::scene::bake::async_tests::{open_editor_in, status};
    let (s, mut editor) = open_editor_in("generate_async_undrawn");
    assert_eq!(s.eval("Lighting.GenerateAsync()").unwrap(), "true");
    let (scene, ctx) = (s.world().scene().clone(), egui::Context::default());
    let mut nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    let mut console = ConsoleLogs::new();
    for frame in 0..4000 {
        let mut sc = scene.borrow_mut();
        editor.frame_work(&ctx, &mut sc, &mut console, &mut nav, false, Paint::Undrawn);
        drop(sc);
        if frame == 0 {
            assert!(!scene.borrow().lighting_bake_requested, "taken at once");
            assert!(status(&s).starts_with("running"), "{}", status(&s));
        }
        if editor.dev.lightmap_bake.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "false");
    assert!(status(&s).starts_with("done"), "{}", status(&s));
}
