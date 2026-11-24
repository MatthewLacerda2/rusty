//! The mark against how the last save was made: the header's, a script's (the same
//! `rusty_edit::save_to_file`, called beside the editor), or none.

use crate::EditorUi;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

fn saved_scene(tag: &str) -> (String, EditorUi, Scene) {
    let dir = rusty_test_support::temp::dir().join(tag);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    rusty_edit::save_to_file(&scene, &path).unwrap();
    let mut editor = EditorUi::new(Default::default());
    assert!(editor.open_scene(&mut scene, &mut ConsoleLogs::new(), &path, false));
    (path, editor, scene)
}

#[test]
fn a_write_outside_the_document_stays_unsaved_until_a_save() {
    let (path, mut editor, mut scene) = saved_scene("unsaved_outside");
    scene.mark_unsaved();
    assert!(editor.unsaved(&mut scene));
    assert!(
        !rusty_edit::has_unsaved_edits(&scene, &path),
        "the document is its file"
    );
    assert!(editor.unsaved(&mut scene), "asking twice clears nothing");
    editor
        .save_scene_to(&scene, &mut ConsoleLogs::new(), &path)
        .unwrap();
    assert!(!editor.unsaved(&mut scene));
}

#[test]
fn a_script_save_clears_the_mark_and_an_edit_after_it_raises_it_again() {
    let (path, mut editor, mut scene) = saved_scene("unsaved_script_save");
    scene.add_entity("A".into());
    scene.mark_unsaved();
    assert!(editor.unsaved(&mut scene));
    rusty_edit::save_to_file(&scene, &path).unwrap();
    assert!(
        !editor.unsaved(&mut scene),
        "a script's save is a full save"
    );
    // Edit, save and edit again between two looks: the last edit stands.
    scene.mark_unsaved();
    rusty_edit::save_to_file(&scene, &path).unwrap();
    scene.mark_unsaved();
    assert!(editor.unsaved(&mut scene));
}

#[test]
fn a_failed_save_clears_nothing() {
    let (path, mut editor, mut scene) = saved_scene("unsaved_failed_save");
    scene.mark_unsaved();
    std::fs::create_dir_all(format!("{path}.lighting.json")).unwrap();
    assert!(editor
        .save_scene_to(&scene, &mut ConsoleLogs::new(), &path)
        .is_err());
    assert!(rusty_edit::save_to_file(&scene, &path).is_err());
    assert!(editor.unsaved(&mut scene));
}
