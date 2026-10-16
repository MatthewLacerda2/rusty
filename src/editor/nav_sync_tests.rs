//! The edit-mode navmesh sync (#992): an edit made outside the inspector (a gizmo
//! drag, a script's `Transform.SetPosition`) reaches the navmesh on the editor's
//! next frame, while play mode leaves the navmesh to its own per-tick sync.

use super::EditorUi;
use crate::navigation::test_support::{add_box, add_floor, move_to};
use crate::navigation::{NavigationGraph, Rebake};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use glam::Vec3;

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

/// Draw one headless editor frame over `scene` and `nav`.
fn draw_frame(scene: &mut Scene, nav: &mut NavigationGraph, mut playing: bool) {
    let ctx = egui::Context::default();
    let mut editor = EditorUi::new();
    let mut console = ConsoleLogs::new();
    ctx.begin_pass(egui::RawInput::default());
    editor.draw(
        &ctx,
        scene,
        &mut console,
        nav,
        &mut playing,
        60.0,
        16.0,
        None,
    );
    let _ = ctx.end_pass();
}

#[test]
fn an_edit_mode_frame_rebakes_what_moved_outside_the_inspector() {
    let (mut scene, mut nav) = moved_crate();
    draw_frame(&mut scene, &mut nav, false);
    assert!(
        matches!(nav.sync(&scene), Rebake::Unchanged),
        "the editor frame should already have synced the moved crate"
    );
}

#[test]
fn a_play_mode_frame_leaves_the_navmesh_to_play_mode() {
    let (mut scene, mut nav) = moved_crate();
    draw_frame(&mut scene, &mut nav, true);
    assert!(matches!(nav.sync(&scene), Rebake::Incremental(_)));
}
