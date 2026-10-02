//! The File menu's scene verbs: new, reset, load and save.

use crate::editor::EditorUi;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Save writes back to the CURRENT scene path (set on load / double-click),
/// falling back to the seeded default scene.
fn current_path(editor: &EditorUi) -> String {
    editor
        .current_scene_path
        .clone()
        .unwrap_or_else(|| crate::scene::DEFAULT_SCENE_PATH.to_string())
}

pub(super) fn save_scene(editor: &mut EditorUi, scene: &Scene, console: &mut ConsoleLogs) {
    let path = current_path(editor);
    match scene.save_to_file(&path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.clone());
            console.info(format!("Scene saved to {}", path));
        }
        Err(err) => console.error(format!("Failed to save scene: {}", err)),
    }
}

pub(super) fn load_scene(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let path = current_path(editor);
    match scene.load_from_file(&path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.clone());
            editor.selected_entity_id = None;
            editor.is_dirty = true;
            console.info(format!("Scene loaded from {}", path));
        }
        Err(err) => console.error(format!("Failed to load scene: {}", err)),
    }
}

/// Start a fresh, empty scene — a blank slate (no entities, default ambient/sky/
/// layers), discarding the in-memory scene. Distinct from Reset Scene, which
/// reverts to the *seeded default* (its ground, lights, etc.). The new scene is
/// unsaved, so its path is cleared until the next Save.
pub(super) fn new_scene(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    *scene = Scene::new();
    editor.current_scene_path = None;
    editor.selected_entity_id = None;
    editor.selected_asset_path = None;
    editor.is_dirty = true;
    console.info("New empty scene".to_string());
}

/// Revert to the seeded default scene (discards the in-memory scene).
pub(super) fn reset_scene(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let path = crate::scene::seed_default_scene();
    match scene.load_from_file(&path) {
        Ok(_) => {
            editor.current_scene_path = Some(path.clone());
            editor.selected_entity_id = None;
            editor.selected_asset_path = None;
            editor.is_dirty = true;
            console.info("Scene reset to default".to_string());
        }
        Err(err) => console.error(format!("Failed to reset scene: {}", err)),
    }
}
