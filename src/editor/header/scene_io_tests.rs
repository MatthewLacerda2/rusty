use super::*;

fn scene_with_one_entity() -> Scene {
    let mut scene = Scene::new();
    scene.add_entity("keep".to_string());
    scene
}

#[test]
fn new_and_reset_do_nothing_in_play() {
    let mut editor = EditorUi::new();
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
    let mut editor = EditorUi::new();
    let mut scene = scene_with_one_entity();
    let mut console = ConsoleLogs::new();
    new_scene(&mut editor, &mut scene, &mut console, false);
    assert_eq!(scene.entity_count(), 0);
    reset_scene(&mut editor, &mut scene, &mut console, false);
    assert!(scene.entity_count() > 0, "the default scene is built");
}
