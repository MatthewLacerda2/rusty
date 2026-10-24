use crate::scene::{save_lighting, LightingSave, Scene, LIGHTING_IN_PLAY};

/// A saved one-entity scene at a fresh path, loaded back as the editor loads it.
fn loaded(tag: &str) -> (Scene, String) {
    let dir = crate::test_temp::dir().join(format!("disk_text_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut authored = Scene::new();
    authored.add_entity("Authored".into());
    authored.save_to_file(&path).unwrap();
    let mut scene = Scene::new();
    scene.load_from_file(&path).unwrap();
    (scene, path)
}

#[test]
fn a_load_and_a_save_are_the_engines_own_text() {
    let (scene, path) = loaded("own");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(scene.is_disk_text(&path, &text), "the load is recorded");
    assert!(!scene.is_disk_text("elsewhere.scene", &text), "by path");
    assert!(!scene.has_unsaved_edits(&path));
    scene.save_to_file(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(scene.is_disk_text(&path, &text), "the save is recorded");
}

/// The editor copies its selection into the scene every frame: not an edit.
#[test]
fn the_selection_is_not_an_unsaved_edit() {
    let (mut scene, path) = loaded("selection");
    scene.selected_entity_id = scene.find_entity_by_name("Authored");
    assert!(!scene.has_unsaved_edits(&path));
    scene.add_entity("Unsaved".into());
    assert!(scene.has_unsaved_edits(&path));
}

/// Generate Lighting rewrites the file's lighting fields: the new text is the
/// engine's own, and edits elsewhere stay unsaved (#832).
#[test]
fn a_lighting_save_is_recorded_and_keeps_edits_unsaved() {
    let (mut scene, path) = loaded("lighting");
    scene.add_entity("Unsaved".into());
    scene.lighting_settings.lightmaps.samples = 7;
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(scene.is_disk_text(&path, &text));
    assert!(
        scene.has_unsaved_edits(&path),
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

/// Generate Lighting over an outside edit the follow has not applied yet: the merged
/// text is not recorded, so the follow still reloads it (#1374).
#[test]
fn a_lighting_save_over_an_outside_edit_is_not_recorded() {
    let (scene, path) = loaded("outside");
    let mut outside = Scene::new();
    outside.add_entity("Outside".into());
    outside.save_to_file(&path).unwrap();
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"Outside\""), "the edit survives the merge");
    assert!(!scene.is_disk_text(&path, &text));
}

/// A load whose sidecar fails still replaced the World, so the document is "what
/// the engine read": a `touch` or rescan is no change (#1374).
#[test]
fn a_load_with_a_broken_sidecar_is_recorded() {
    let (_, path) = loaded("sidecar");
    std::fs::write(crate::scene::sidecar_path(&path), "{ broken").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let mut fresh = Scene::new();
    assert!(fresh.load_from_file(&path).unwrap_err().replaced_world());
    assert!(fresh.is_disk_text(&path, &text));
    assert!(!fresh.has_unsaved_edits(&path));
}

/// Reset Scene: the record is the file's text whatever it holds, so a `touch` or
/// the editor's own Generate Lighting is no change; the rebuilt World is unsaved
/// against it (#1374).
#[test]
fn adopting_a_file_records_its_text() {
    let (mut scene, path) = loaded("adopt");
    let text = std::fs::read_to_string(&path).unwrap();
    scene.add_entity("Rebuilt".into());
    scene.adopt_disk_text(&path);
    assert!(scene.is_disk_text(&path, &text));
    assert!(scene.has_unsaved_edits(&path), "the rebuilt World differs");
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let lit = std::fs::read_to_string(&path).unwrap();
    assert!(
        scene.is_disk_text(&path, &lit),
        "the bake is the engine's own"
    );
}
