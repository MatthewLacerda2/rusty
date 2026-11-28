//! Several loaded scenes: the first scene's PVS under its file's ids, the unsaved mark after a
//! Save As, and every loaded scene read again after a bake.

use super::tests::{files, open};
use super::{any_unsaved, reopen_loaded, save_all, save_as};
use glam::{UVec3, Vec3};
use rusty_scene::lighting::occlusion::{BakedObject, Grid, OcclusionData};

fn object(entity: u32) -> BakedObject {
    BakedObject {
        entity,
        fingerprint: u64::from(entity),
        occluder: false,
    }
}

#[test]
fn the_first_scenes_pvs_holds_only_its_members_under_its_file_ids() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let player = scene.find_entity_by_name("Player").unwrap();
    let door = scene.find_entity_by_name("Door").unwrap();
    let lamp = scene.add_entity("Lamp".into());
    let grid = Grid {
        origin: Vec3::ZERO,
        cell: 1.0,
        dims: UVec3::new(2, 1, 1),
    };
    // Row 0 sees all three; row 1 the door and the lamp, not the player.
    let objects = vec![object(player), object(door), object(lamp)];
    let pvs = OcclusionData::new(grid, objects, vec![0b111, 0b110], vec![0, 1]).unwrap();
    scene.put_baked_occlusion(Some(pvs));
    save_all(&scene, Some(&base)).unwrap();

    let mut alone = rusty_scene::Scene::new();
    crate::load_from_file(&mut alone, &base).unwrap();
    let file_id = |name: &str| alone.find_entity_by_name(name).unwrap();
    let (player, lamp) = (file_id("Player"), file_id("Lamp"));
    let read = alone.occlusion.as_deref().expect("the base's PVS");
    let entities: Vec<u32> = read.objects.iter().map(|o| o.entity).collect();
    assert_eq!(entities, [player, lamp], "no level entity, file ids only");
    let (p, l) = (
        read.object_of(player).unwrap(),
        read.object_of(lamp).unwrap(),
    );
    assert!(read.sees(0, p) && read.sees(0, l));
    assert!(
        !read.sees(1, p) && read.sees(1, l),
        "the bits moved with them"
    );
}

#[test]
fn save_as_with_several_loaded_clears_the_unsaved_mark() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let level_handle = scene.loaded.handle_of(&level).unwrap();
    scene.set_active_scene(level_handle);
    let door = scene.find_entity_by_name("Door").unwrap();
    scene.world.set_name(door, "Gate".into());
    assert!(any_unsaved(&scene));
    scene.take_saved();
    let renamed = dir.join("level2.scene").to_string_lossy().into_owned();
    save_as(&mut scene, &renamed).unwrap();
    assert!(!any_unsaved(&scene), "every file is clean");
    assert_eq!(scene.take_saved(), Some(false), "the editor's mark clears");
}

#[test]
fn reopen_loaded_reads_every_scene_again_and_keeps_the_active_one() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let level_handle = scene.loaded.handle_of(&level).unwrap();
    scene.set_active_scene(level_handle);
    let warnings = reopen_loaded(&mut scene).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let paths: Vec<Option<String>> = scene.loaded.paths(None);
    assert_eq!(paths, [Some(base.clone()), Some(level.clone())]);
    let active = scene.loaded.path_of(scene.loaded.active());
    assert_eq!(active, Some(level.as_str()));
    assert!(scene.find_entity_by_name("Door").is_some());
    assert!(scene.find_entity_by_name("Player").is_some());
    assert!(!any_unsaved(&scene));
}
