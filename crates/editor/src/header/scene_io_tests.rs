use super::*;

fn scene_with_one_entity() -> Scene {
    let mut scene = Scene::new();
    scene.add_entity("keep".to_string());
    scene
}

#[test]
fn new_and_reset_do_nothing_in_play() {
    let mut editor = EditorUi::new(Default::default());
    let mut scene = scene_with_one_entity();
    let before = scene.entity_count();
    let mut console = ConsoleLogs::new();
    new_scene(&mut editor, &mut scene, &mut console, true);
    reset_scene(&mut editor, &mut scene, &mut console, true);
    assert_eq!(scene.entity_count(), before, "the World is untouched");
    assert_eq!(console.messages.len(), 2, "each refusal says why");
}

#[test]
fn new_and_reset_work_in_edit_mode() {
    let mut editor = EditorUi::new(Default::default());
    let mut scene = scene_with_one_entity();
    let mut console = ConsoleLogs::new();
    new_scene(&mut editor, &mut scene, &mut console, false);
    assert_eq!(scene.entity_count(), 0);
    reset_scene(&mut editor, &mut scene, &mut console, false);
    assert!(scene.entity_count() > 0, "the default scene is built");
}

/// A scripted `Scene.New()` in an attached editor (#2000): the path cell is the
/// editor's, so the header shows an unsaved "Untitled", and the scene's identity is
/// new, which is what the shell's follow reads to drop the selection and the
/// unsaved mark, as it does for a scripted `Scene.Load`.
#[test]
fn a_scripted_new_is_what_the_editor_shows() {
    use std::{cell::RefCell, rc::Rc};
    let mut game = rusty_app::GameWorld::new(
        Rc::new(RefCell::new(scene_with_one_entity())),
        Rc::new(RefCell::new(rusty_core::input::InputState::new())),
        Rc::new(RefCell::new(rusty_navigation::NavigationGraph::new(
            -8.0, 8.0, -8.0, 8.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    let mut editor = EditorUi::new(game.script_manager().scene_path_cell());
    editor.set_current_scene_path(Some("levels/a.scene".to_string()));
    game.init_edit_runtime().unwrap();
    let before = game.scene().borrow().id();
    game.script_manager().eval("Scene.New()").unwrap();
    assert_eq!(editor.current_scene_path(), None);
    assert_eq!(editor.scene_name(), "Untitled");
    let scene = game.scene().borrow();
    assert_eq!(scene.entity_count(), 0);
    assert_ne!(scene.id(), before, "the follow sees a swapped scene");
}
