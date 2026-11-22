use crate::{save_lighting, LightingSave, LIGHTING_IN_PLAY};
use rusty_scene::Scene;

fn loaded(tag: &str) -> (Scene, String) {
    let dir = rusty_test_support::temp::dir().join(format!("disk_text_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut authored = Scene::new();
    authored.add_entity("Authored".into());
    crate::save_to_file(&authored, &path).unwrap();
    let mut scene = Scene::new();
    crate::load_from_file(&mut scene, &path).unwrap();
    (scene, path)
}

#[test]
fn a_load_and_a_save_are_the_engines_own_text() {
    let (scene, path) = loaded("own");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(scene.is_disk_text(&path, &text), "the load is recorded");
    assert!(!scene.is_disk_text("elsewhere.scene", &text), "by path");
    assert!(!crate::has_unsaved_edits(&scene, &path));
    crate::save_to_file(&scene, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(scene.is_disk_text(&path, &text), "the save is recorded");
}

#[test]
fn the_selection_is_not_an_unsaved_edit() {
    let (mut scene, path) = loaded("selection");
    scene.selected_entity_id = scene.find_entity_by_name("Authored");
    assert!(!crate::has_unsaved_edits(&scene, &path));
    scene.add_entity("Unsaved".into());
    assert!(crate::has_unsaved_edits(&scene, &path));
}

#[test]
fn a_workspace_script_by_absolute_path_is_no_unsaved_edit() {
    let (mut scene, path) = loaded("script");
    let id = scene.find_entity_by_name("Authored").unwrap();
    let script = std::env::current_dir()
        .unwrap()
        .join("assets")
        .join("x.lua");
    scene
        .world
        .scripts_mut(id)
        .unwrap()
        .push(rusty_scene::ScriptComponent {
            path: script.to_string_lossy().into_owned(),
            ..Default::default()
        });
    assert!(
        crate::has_unsaved_edits(&scene, &path),
        "attaching it is an edit"
    );
    crate::save_to_file(&scene, &path).unwrap();
    assert!(!crate::has_unsaved_edits(&scene, &path), "saved, relative");
}

#[test]
fn a_lighting_save_is_recorded_and_keeps_edits_unsaved() {
    let (mut scene, path) = loaded("lighting");
    scene.add_entity("Unsaved".into());
    scene.lighting_settings.lightmaps.samples = 7;
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(scene.is_disk_text(&path, &text));
    assert!(
        crate::has_unsaved_edits(&scene, &path),
        "the entity is still unsaved"
    );
}

#[test]
fn a_lighting_save_is_refused_in_play() {
    let (mut scene, path) = loaded("play");
    let before = std::fs::read(&path).unwrap();
    scene.set_playing(true);
    let refused = save_lighting(&scene, &path);
    assert_eq!(refused, Err(LIGHTING_IN_PLAY.to_string()));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file is untouched"
    );
}

#[test]
fn a_lighting_save_over_an_outside_edit_is_not_recorded() {
    let (scene, path) = loaded("outside");
    let mut outside = Scene::new();
    outside.add_entity("Outside".into());
    crate::save_to_file(&outside, &path).unwrap();
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"Outside\""), "the edit survives the merge");
    assert!(!scene.is_disk_text(&path, &text));
}

#[test]
fn a_load_with_a_broken_sidecar_is_recorded() {
    let (_, path) = loaded("sidecar");
    std::fs::write(rusty_scene::sidecar_path(&path), "{ broken").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let mut fresh = Scene::new();
    assert!(crate::load_from_file(&mut fresh, &path)
        .unwrap_err()
        .replaced_world());
    assert!(fresh.is_disk_text(&path, &text));
    assert!(!crate::has_unsaved_edits(&fresh, &path));
}

#[test]
fn adopting_a_file_records_its_text() {
    let (mut scene, path) = loaded("adopt");
    let text = std::fs::read_to_string(&path).unwrap();
    scene.add_entity("Rebuilt".into());
    scene.adopt_disk_text(&path);
    assert!(scene.is_disk_text(&path, &text));
    assert!(
        crate::has_unsaved_edits(&scene, &path),
        "the rebuilt World differs"
    );
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let lit = std::fs::read_to_string(&path).unwrap();
    assert!(
        scene.is_disk_text(&path, &lit),
        "the bake is the engine's own"
    );
}
