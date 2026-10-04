use super::scene_json;
use crate::scene::{Scene, ScriptComponent};

/// A script attached by absolute path inside the workspace saves relative to it,
/// `/`-separated (#783), and one outside it saves as it was.
#[test]
fn a_saved_scene_names_workspace_scripts_relative_to_the_workspace() {
    let root = std::env::current_dir().unwrap();
    let inside = root
        .join("project")
        .join("assets")
        .join("scripts")
        .join("x.lua");
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
    assert!(
        json.contains(r#""path": "project/assets/scripts/x.lua""#),
        "{json}"
    );
    assert!(json.contains(outside), "{json}");
    assert!(!json.contains(&*root.to_string_lossy()), "{json}");
}
