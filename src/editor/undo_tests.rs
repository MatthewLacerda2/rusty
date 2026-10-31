//! Drawing the editor is no edit (#1631): the Inspector borrows a selected
//! entity's components mutably every frame to show them, and a frame that only
//! shows must leave the Undo history as it was, whatever the entity carries.

use super::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::authoring::{add_component, create_entity, ComponentKind, Primitive};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// One editor frame with no input, recorded as a full edit.
fn idle_frame(editor: &mut EditorUi, scene: &mut Scene) {
    let ctx = egui::Context::default();
    let mut nav = NavigationGraph::from_scene(scene);
    let (mut console, mut playing) = (ConsoleLogs::new(), false);
    scene.begin_edits(); // the strict path: the scene level is compared too
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
    scene.commit_edits(None);
}

#[test]
fn showing_a_fully_dressed_entity_records_no_step() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Everything", Some(Primitive::Box));
    let kinds = [
        ComponentKind::Light,
        ComponentKind::Collider,
        ComponentKind::RigidBody,
        ComponentKind::Texture,
        ComponentKind::NavMeshAgent,
        ComponentKind::Camera,
        ComponentKind::Particles,
        ComponentKind::PostProcessing,
        ComponentKind::Audio,
        ComponentKind::Animator,
        ComponentKind::Trail,
    ];
    for kind in kinds {
        add_component(&mut scene, id, kind);
    }
    let mut editor = EditorUi::new(Default::default());
    editor.selected_entity_id = Some(id);
    for _ in 0..3 {
        idle_frame(&mut editor, &mut scene);
    }
    assert_eq!(
        scene.undo_name(),
        None,
        "a frame that only shows edits nothing"
    );
    editor.selected_entity_id = None; // the Scene Settings view
    idle_frame(&mut editor, &mut scene);
    assert_eq!(scene.undo_name(), None);
}
