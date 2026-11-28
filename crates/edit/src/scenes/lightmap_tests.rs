//! The first scene's lightmaps, saved beside other loaded scenes: only its members' ids reach
//! its file, under the file's own ids.

use super::save_all;
use super::tests::{files, open};
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::read_scene_file;

#[test]
fn the_first_scenes_lightmaps_hold_only_its_members_under_its_file_ids() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let door = scene.world.find_by_name("Door").unwrap();
    let lamp = scene.add_entity("Lamp".into());
    scene.lightmaps.insert(LightmapEntry {
        entity: lamp,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    scene.lightmaps.lights.extend([door, lamp]);
    save_all(&scene, Some(&base)).unwrap();
    let saved = read_scene_file(&base).unwrap().lightmaps;
    let file_lamp = read_scene_file(&base).unwrap().entities;
    let file_lamp = file_lamp.iter().find(|e| e.name == "Lamp").unwrap().id;
    let entries: Vec<u32> = saved.entries().map(|e| e.entity).collect();
    assert_eq!(entries, [file_lamp], "the lamp's entry, under its file id");
    let lights: Vec<u32> = saved.lights.iter().copied().collect();
    assert_eq!(
        lights,
        [file_lamp],
        "the level's door is no light of the base's file"
    );
}
