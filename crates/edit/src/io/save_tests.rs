use super::{save_to_file, scene_json, SAVE_IN_PLAY};
use rusty_scene::{Scene, ScriptComponent};

#[test]
fn a_saved_scene_names_workspace_scripts_relative_to_the_workspace() {
    let root = std::env::current_dir().unwrap();
    let inside = root.join("assets").join("scripts").join("x.lua");
    let outside = "/elsewhere/y.lua";
    let mut scene = Scene::new();
    let id = scene.add_entity("Scripted".to_string());
    for path in [inside.to_string_lossy().into_owned(), outside.to_string()] {
        scene.world.scripts_mut(id).unwrap().push(ScriptComponent {
            path,
            ..Default::default()
        });
    }
    let json = scene_json(&scene).unwrap();
    assert!(json.contains(r#""path": "assets/scripts/x.lua""#), "{json}");
    assert!(json.contains(outside), "{json}");
    assert!(!json.contains(&*root.to_string_lossy()), "{json}");
}

#[test]
fn a_playing_scene_is_never_saved() {
    let path = rusty_test_support::temp::dir().join("authored.scene");
    let path = path.to_string_lossy().into_owned();
    let mut scene = Scene::new();
    scene.add_entity("Authored".to_string());
    save_to_file(&scene, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    scene.set_playing(true);
    scene.add_entity("Bullet".to_string());
    assert_eq!(save_to_file(&scene, &path), Err(SAVE_IN_PLAY.to_string()));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file is untouched"
    );
    scene.set_playing(false);
    assert!(
        save_to_file(&scene, &path).is_ok(),
        "saves again after Stop"
    );
}

#[test]
fn a_save_over_a_scene_leaves_the_new_bytes_and_no_temp_file() {
    let dir = rusty_test_support::temp::dir();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    save_to_file(&scene, &path).unwrap();
    scene.add_entity("Added".to_string());
    save_to_file(&scene, &path).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        scene_json(&scene).unwrap()
    );
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(
        names.iter().all(|n| !n.to_string_lossy().ends_with(".tmp")),
        "{names:?}"
    );
}

#[test]
fn a_failed_save_leaves_the_old_scene_whole() {
    use std::os::unix::fs::PermissionsExt;
    let dir = rusty_test_support::temp::dir().join("locked");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    scene.add_entity("Authored".to_string());
    save_to_file(&scene, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mode = |m| std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(m));
    mode(0o555).unwrap();
    let probe = dir.join("probe");
    if std::fs::write(&probe, "").is_ok() {
        let _ = std::fs::remove_file(probe);
        mode(0o755).unwrap();
        eprintln!("skipped: this user writes into a read-only folder");
        return;
    }
    scene.add_entity("Unsaved".to_string());
    let result = save_to_file(&scene, &path);
    mode(0o755).unwrap();
    assert!(result.is_err(), "the save reports the failure");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the old scene is whole"
    );
}
