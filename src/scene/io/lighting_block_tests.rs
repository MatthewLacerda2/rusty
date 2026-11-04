use super::{save_lighting, LightingSave};
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::Scene;

/// A scratch folder per test, so parallel tests never share a scene file.
fn scratch(name: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("lighting_block_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

#[test]
fn lighting_is_written_and_unsaved_edits_stay_unsaved() {
    let path = scratch("written");
    let mut scene = Scene::new();
    let saved = scene.add_entity("Saved".into());
    scene.save_to_file(&path).unwrap();

    scene.add_entity("Unsaved".into());
    scene.lighting_settings.lightmaps.samples = 7;
    let set = &mut scene.lightmaps;
    set.pages
        .push("level.scene.lightmaps/lightmap_0.png".into());
    set.entries.push(LightmapEntry {
        entity: saved,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));

    let mut back = Scene::new();
    back.load_from_file(&path).unwrap();
    assert_eq!(back.lighting_settings.lightmaps.samples, 7);
    assert_eq!(back.lightmaps, scene.lightmaps);
    assert!(back.find_entity_by_name("Saved").is_some());
    assert!(back.find_entity_by_name("Unsaved").is_none());
}

#[test]
fn the_file_reads_as_a_full_save_of_the_same_document_would() {
    let path = scratch("layout");
    let mut scene = Scene::new();
    scene.add_entity("Crate".into());
    scene.lighting_settings.lightmaps.directional = false;
    scene.save_to_file(&path).unwrap();
    let full = std::fs::read_to_string(&path).unwrap();
    save_lighting(&scene, &path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), full);
}

#[test]
fn a_document_loading_would_change_is_left_alone() {
    let path = scratch("hand_edited");
    let mut scene = Scene::new();
    scene.save_to_file(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let edited = text.replacen('{', "{\n  \"retired_field\": 1,", 1);
    std::fs::write(&path, &edited).unwrap();

    scene.lighting_settings.lightmaps.bounces = 9;
    assert_eq!(
        save_lighting(&scene, &path),
        Ok(LightingSave::NeedsSceneSave)
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
}

#[test]
fn a_missing_file_is_an_error() {
    let path = scratch("missing").replace("level.scene", "absent.scene");
    assert!(save_lighting(&Scene::new(), &path).is_err());
}

/// A scene saved before the diffuse model existed (#1698), with a custom knob so its
/// settings block is in the file: its first Generate still writes the lighting.
#[test]
fn a_settings_block_without_the_diffuse_model_is_written_and_takes_it() {
    let path = scratch("pre_diffuse");
    let mut scene = Scene::new();
    scene.lighting_settings.lightmaps.samples = 7;
    scene.save_to_file(&path).unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let block = doc["lighting_settings"].as_object_mut().unwrap();
    block.remove("diffuse_model");
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));

    let wrap = crate::scene::lighting::DiffuseModel::HalfLambert;
    scene.lighting_settings.diffuse_model = wrap;
    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    let mut back = Scene::new();
    back.load_from_file(&path).unwrap();
    assert_eq!(back.lighting_settings.diffuse_model, wrap);
}
