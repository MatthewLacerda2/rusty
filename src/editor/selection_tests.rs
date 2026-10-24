//! The editor's selection never names a despawned entity (#1264): a destroy made
//! outside the editor (a script's `Scene.DestroyEntity`, a re-import) is not undone
//! by the editor pushing its own copy back into the scene on the next frame.

use super::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

fn draw_frame(editor: &mut EditorUi, scene: &mut Scene) {
    let ctx = egui::Context::default();
    let mut nav = NavigationGraph::from_scene(scene);
    let mut console = ConsoleLogs::new();
    let mut playing = false;
    ctx.begin_pass(egui::RawInput::default());
    editor.draw(
        &ctx,
        scene,
        &mut console,
        &mut nav,
        &mut playing,
        60.0,
        16.0,
        None,
    );
    ctx.end_pass().textures_delta.clear();
}

#[test]
fn a_destroyed_selection_stays_cleared_after_the_next_frame() {
    let mut scene = Scene::new();
    let doomed = scene.add_entity("doomed".into());
    let mut editor = EditorUi::new(Default::default());
    editor.selected_entity_id = Some(doomed);
    draw_frame(&mut editor, &mut scene);
    assert_eq!(scene.selected_entity_id, Some(doomed));

    scene.destroy_entity(doomed);
    draw_frame(&mut editor, &mut scene);
    assert_eq!(editor.selected_entity_id, None);
    assert_eq!(scene.selected_entity_id, None);
}

#[test]
fn a_live_selection_survives_the_frame() {
    let mut scene = Scene::new();
    let kept = scene.add_entity("kept".into());
    let mut editor = EditorUi::new(Default::default());
    editor.selected_entity_id = Some(kept);
    draw_frame(&mut editor, &mut scene);
    assert_eq!(editor.selected_entity_id, Some(kept));
}
