//! The default scene seeds through the manifest (#746), which only works if building
//! it twice gives the same bytes; and Reset Scene rebuilds it rather than reloading.

use super::super::scene_json;
use crate::scene::{default_scene, Scene};

fn built_json() -> String {
    let mut scene = Scene::new();
    default_scene::build(&mut scene, default_scene::BOT_SCRIPT);
    scene_json(&scene).expect("the default scene serializes")
}

#[test]
fn the_default_scene_serializes_to_the_same_bytes_every_build() {
    // Otherwise every boot would see a "newer default" and never trust its record.
    assert_eq!(built_json(), built_json());
}

#[test]
fn building_the_default_scene_replaces_what_was_there() {
    let mut scene = Scene::new();
    crate::scene::authoring::create_entity(&mut scene, "Stray", None);
    for _ in 0..20 {
        crate::scene::authoring::create_entity(&mut scene, "Filler", None);
    }
    let path = super::build_default_scene(&mut scene);
    assert_eq!(path, super::DEFAULT_SCENE_PATH);
    // No stray entity, no shifted ids: exactly the freshly built default.
    assert_eq!(scene_json(&scene).unwrap(), built_json());
}

#[test]
fn only_bundled_lua_scripts_are_seeded_and_recorded() {
    let root = crate::test_temp::dir().join("seed_scripts");
    std::fs::remove_dir_all(&root).ok();
    let (source, dest) = (root.join("bundled"), root.join("project").join("scripts"));
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("bot.lua"), "-- brain").unwrap();
    std::fs::write(source.join("notes.txt"), "not a script").unwrap();
    let files = super::bundled_scripts(&source, &dest);
    let manifest = root.join(".seeded");
    super::seed_files(&manifest, &files, "delete it");
    assert_eq!(
        std::fs::read_to_string(dest.join("bot.lua")).unwrap(),
        "-- brain"
    );
    assert!(!dest.join("notes.txt").exists());
    let record = std::fs::read_to_string(&manifest).unwrap();
    assert!(
        record.contains("bot.lua") && !record.contains("notes"),
        "{record}"
    );
}

#[test]
fn the_seeded_default_scene_loads_back() {
    let root = crate::test_temp::dir().join("seed_scene");
    std::fs::remove_dir_all(&root).ok();
    let path = root.join("default.scene");
    let files = [(path.clone(), super::default_scene_bytes().unwrap())];
    super::seed_files(&root.join(".seeded"), &files, "reset");
    let data = super::super::read_scene_file(path.to_str().unwrap()).expect("loads");
    assert!(!data.entities.is_empty());
}

#[test]
fn scene_paths_are_told_by_their_extension() {
    assert!(super::super::is_scene_path("project/scenes/default.scene"));
    assert!(super::super::is_scene_path("A.SCENE"));
    assert!(!super::super::is_scene_path(
        "project/assets/scripts/bot.lua"
    ));
    assert!(!super::super::is_scene_path("scene"));
}
