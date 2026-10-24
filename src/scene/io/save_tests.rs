use super::{save_to_file, scene_json, SAVE_IN_PLAY};
use crate::scene::{Scene, ScriptComponent};

/// A script attached by absolute path inside the project saves relative to it,
/// `/`-separated (#783), and one outside it saves as it was.
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

/// Save is refused in Play (#1340): the World holds the play state, and the file
/// on disk is left byte for byte as it was.
#[test]
fn a_playing_scene_is_never_saved() {
    let path = crate::test_temp::dir().join("authored.scene");
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
