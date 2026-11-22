//! stored-format: the scene file as read (`read_scene_file`), its bake files (`bake_files`)

mod bake_files;
mod disk_text;

pub use bake_files::{Action, BakeFiles, Saved};
pub use disk_text::DiskText;

use crate::serialize::SceneData;
use crate::Scene;

pub const SCENE_EXTENSION: &str = "scene";

/// Refused in Play, as Unity greys File ▸ New out: the swap would skip every script's
/// `OnDisable` and `OnDestroy`.
pub fn new_scene(scene: &mut Scene, current: &mut Option<String>) -> Result<(), &'static str> {
    if scene.is_playing() {
        return Err(NEW_IN_PLAY);
    }
    scene.clear();
    *current = None;
    Ok(())
}

pub const NEW_IN_PLAY: &str = "a new scene cannot be started during Play: stop Play first";

/// Touches no live scene, so a caller can fail before tearing the current one down.
pub fn read_scene_file(path: &str) -> Result<SceneData, String> {
    parse_scene_text(&read_text(path)?, path)
}

pub fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read scene file {}: {}", path, e))
}

pub fn parse_scene_text(json: &str, path: &str) -> Result<SceneData, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to deserialize scene {}: {}", path, e))
}
