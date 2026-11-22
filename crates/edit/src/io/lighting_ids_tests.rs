//! A file's lightmap entries name only entities it holds (`serialize::file_lightmaps`).

use super::{save_lighting, LightingSave};
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::Scene;

fn scratch(name: &str) -> String {
    let dir = rusty_test_support::temp::dir().join(format!("lighting_ids_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

fn entry(entity: u32) -> LightmapEntry {
    LightmapEntry {
        entity,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    }
}

fn written_entities(path: &str) -> Vec<u32> {
    let data = super::parse_scene_text(&std::fs::read_to_string(path).unwrap(), path).unwrap();
    data.lightmaps.entries().map(|e| e.entity).collect()
}

#[test]
fn the_bake_write_names_only_entities_the_file_holds() {
    let path = scratch("file_ids");
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".into());
    crate::save_to_file(&scene, &path).unwrap();
    let unsaved = scene.add_entity("Unsaved".into());
    let gone = scene.add_entity("Gone".into());
    scene.destroy_entity(gone);
    // A bake that finished after the delete still holds the deleted entity's entry.
    scene.lightmaps.extend([wall, gone, unsaved].map(entry));

    assert_eq!(save_lighting(&scene, &path), Ok(LightingSave::Written));
    assert_eq!(written_entities(&path), [wall]);
    assert_eq!(
        scene.lightmaps.entries().count(),
        3,
        "the scene's set stays whole"
    );
}

#[test]
fn an_undone_delete_finds_its_entry_after_the_bake_write() {
    let path = scratch("undo");
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".into());
    scene.lightmaps.insert(entry(wall));
    crate::save_to_file(&scene, &path).unwrap();
    scene.begin_edits();
    scene.destroy_entity(wall);
    assert!(scene.commit_edits(None));

    save_lighting(&scene, &path).unwrap();
    assert!(written_entities(&path).is_empty(), "no live entry to write");
    scene.undo().expect("a step to undo");
    assert_eq!(scene.lightmaps.get(wall), Some(&entry(wall)));
}

#[test]
fn an_old_files_entry_for_an_entity_it_does_not_hold_is_dropped_on_load() {
    let path = scratch("old_file");
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".into());
    let mut data = crate::to_scene_data(&scene);
    data.lightmaps.extend([wall, wall + 1].map(entry));
    std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();

    let mut back = Scene::new();
    let said =
        rusty_test_support::logs::during(|| crate::load_from_file(&mut back, &path).unwrap());
    assert_eq!(back.lightmaps.get(wall), Some(&entry(wall)));
    let fresh = back.add_entity("Fresh".into());
    assert_eq!(fresh, wall + 1, "the load hands the dead entry's id out");
    assert_eq!(back.lightmaps.get(fresh), None);
    let warned = said.iter().filter(|l| l.contains("does not hold")).count();
    assert_eq!(warned, 1, "{said:?}");
}
