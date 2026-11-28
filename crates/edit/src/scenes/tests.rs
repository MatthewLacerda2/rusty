//! Two scene files loaded into one World in edit mode, each saved back to its own file.

use super::{join_additive, save_all};
use crate::load::prepare::{PreparedScene, SceneSidecars};
use crate::{apply_scene_data, has_unsaved_edits, load_from_file, save_to_file, to_scene_data};
use rusty_components::{JointComponent, MaterialAsset, MaterialComponent};
use rusty_scene::rehydrate::RehydratedScene;
use rusty_scene::Scene;
use rusty_test_support::logs;

/// Saves `scene` alone to `name` in `dir`, as a one-scene editor would.
fn write(dir: &std::path::Path, name: &str, scene: &Scene) -> String {
    let path = dir.join(name).to_string_lossy().into_owned();
    save_to_file(scene, &path).unwrap();
    path
}

/// `base.scene`: a player (1) and a gun (2) painted red `Paint`. `level.scene`: a door (1)
/// hinged on its frame (2), the frame painted blue `Paint`: the join renames it.
pub(super) fn files(dir: &std::path::Path) -> (String, String) {
    let painted = |scene: &mut Scene, id: u32, metallic: f32| {
        let mut data = to_scene_data(scene);
        let paint = MaterialAsset {
            metallic,
            ..Default::default()
        };
        data.materials.insert("Paint".into(), paint);
        let entity = data.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.material = Some(MaterialComponent::new("Paint"));
        apply_scene_data(scene, data);
    };
    let mut base = Scene::new();
    base.add_entity("Player".into());
    let gun = base.add_entity("Gun".into());
    painted(&mut base, gun, 0.0);
    let mut level = Scene::new();
    let door = level.add_entity("Door".into());
    let frame = level.add_entity("Frame".into());
    let hinge = JointComponent {
        connected_body: Some(frame),
        ..Default::default()
    };
    level.world.set_joint(door, Some(hinge));
    painted(&mut level, frame, 0.75);
    (
        write(dir, "base.scene", &base),
        write(dir, "level.scene", &level),
    )
}

/// The editor's view: `base` opened, `level` loaded beside it.
pub(super) fn open(base: &str, level: &str) -> Scene {
    let mut scene = Scene::new();
    load_from_file(&mut scene, base).unwrap();
    let text = std::fs::read_to_string(level).unwrap();
    let data = rusty_scene::parse_scene_text(&text, level).unwrap();
    let document = RehydratedScene::new(data);
    let prepared = PreparedScene::new(level.into(), document, SceneSidecars::read(level));
    join_additive(&mut scene, Some(base.into()), prepared);
    scene.record_disk_text_also(level, text);
    scene
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn an_untouched_pair_saves_byte_identical() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let (base_before, level_before) = (read(&base), read(&level));
    let scene = open(&base, &level);
    assert!(
        scene.materials.contains_key("Paint (1)"),
        "the join renamed the level's paint"
    );
    assert!(!has_unsaved_edits(&scene, &base));
    assert!(!has_unsaved_edits(&scene, &level));
    let saved = save_all(&scene, Some(&base)).unwrap();
    assert_eq!(saved, [base.clone(), level.clone()]);
    assert_eq!(read(&base), base_before);
    assert_eq!(read(&level), level_before);
}

#[test]
fn an_edit_to_one_scene_writes_only_its_file() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let level_before = read(&level);
    let mut scene = open(&base, &level);
    let door = scene.find_entity_by_name("Door").unwrap();
    let player = scene.find_entity_by_name("Player").unwrap();
    scene.world.set_name(player, "Hero".into());
    assert!(has_unsaved_edits(&scene, &base));
    assert!(!has_unsaved_edits(&scene, &level), "the level is untouched");
    save_all(&scene, Some(&base)).unwrap();
    assert_eq!(read(&level), level_before);
    let written = rusty_scene::read_scene_file(&base).unwrap();
    let names: Vec<&str> = written.entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Hero", "Gun"], "no door: {door} is the level's");
    assert!(!has_unsaved_edits(&scene, &base));
}

#[test]
fn a_new_entity_takes_an_id_past_its_files_own() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let level_handle = scene.loaded.handle_of(&level).unwrap();
    scene.set_active_scene(level_handle);
    let lamp = scene.add_entity("Lamp".into());
    assert!(lamp > 4, "the World's id is past both files'");
    save_to_file(&scene, &level).unwrap();
    let written = rusty_scene::read_scene_file(&level).unwrap();
    let ids: Vec<(String, u32)> = written
        .entities
        .iter()
        .map(|e| (e.name.clone(), e.id))
        .collect();
    assert_eq!(
        ids,
        [("Door".into(), 1), ("Frame".into(), 2), ("Lamp".into(), 3)]
    );
    assert_eq!(written.next_entity_id, 4);
    let door = written.entities.iter().find(|e| e.name == "Door").unwrap();
    let hinge = door.joint.as_ref().and_then(|j| j.connected_body);
    assert_eq!(hinge, Some(2), "the hinge names the frame by the file's id");
    let frame = written.entities.iter().find(|e| e.name == "Frame").unwrap();
    let paint = frame.material.as_ref().unwrap().materials.clone();
    assert_eq!(paint, ["Paint"], "the file's own name, not the World's");
    assert_eq!(written.materials["Paint"].metallic, 0.75);
}

#[test]
fn a_reference_into_another_scene_is_not_written_and_warned() {
    logs::init();
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let door = scene.find_entity_by_name("Door").unwrap();
    let gun = scene.find_entity_by_name("Gun").unwrap();
    let hinge = JointComponent {
        connected_body: Some(gun),
        ..Default::default()
    };
    scene.world.set_joint(door, Some(hinge));
    logs::take();
    save_to_file(&scene, &level).unwrap();
    let written = rusty_scene::read_scene_file(&level).unwrap();
    let door = written.entities.iter().find(|e| e.name == "Door").unwrap();
    assert_eq!(door.joint.as_ref().and_then(|j| j.connected_body), None);
    let said = logs::take().join("\n");
    assert!(said.contains("'Door'") && said.contains("'Gun'"), "{said}");
    assert!(
        said.contains(&base),
        "names the scene the gun is in: {said}"
    );
}

#[test]
fn move_to_moves_a_subtree_and_both_files_show_it() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let mut scene = open(&base, &level);
    let player = scene.find_entity_by_name("Player").unwrap();
    let gun = scene.find_entity_by_name("Gun").unwrap();
    scene.set_parent(gun, Some(player)).unwrap();
    let level_handle = scene.loaded.handle_of(&level).unwrap();
    scene.move_to_scene(player, level_handle).unwrap();
    save_all(&scene, Some(&base)).unwrap();
    let names = |path: &str| -> Vec<String> {
        let data = rusty_scene::read_scene_file(path).unwrap();
        data.entities.iter().map(|e| e.name.clone()).collect()
    };
    assert!(names(&base).is_empty(), "the player and its gun left");
    assert_eq!(names(&level), ["Door", "Frame", "Player", "Gun"]);
    let written = rusty_scene::read_scene_file(&level).unwrap();
    let gun = written.entities.iter().find(|e| e.name == "Gun").unwrap();
    let player = written
        .entities
        .iter()
        .find(|e| e.name == "Player")
        .unwrap();
    assert_eq!(
        gun.parent_id,
        Some(player.id),
        "the parent link follows the new ids"
    );
    assert_eq!(
        written.materials["Paint (1)"].metallic, 0.0,
        "the gun's red paint, uniquified"
    );
}

#[test]
fn a_save_to_a_path_no_scene_holds_errors_naming_the_loaded_ones() {
    let dir = rusty_test_support::temp::dir();
    let (base, level) = files(&dir);
    let scene = open(&base, &level);
    let elsewhere = dir.join("other.scene").to_string_lossy().into_owned();
    let why = save_to_file(&scene, &elsewhere).unwrap_err();
    assert!(
        why.contains("not one of the loaded scenes") && why.contains(&level),
        "{why}"
    );
}
