//! crates/scene/src/io/mod.rs — the scene file's base half: its extension, reading
//! its document, the record of the text at it and the bake files beside it.
//!
//! Saving, loading into a live scene and seeding the default scene are the edit
//! side's (`rusty_edit::io`): a save converts the World through `to_scene_data`, a
//! load through `apply_scene_data`.
//!
//! stored-format: the scene file's text as read (`read_scene_file`), the bake files
//! beside it (`bake_files`) and the record of the engine's own text (`disk_text`)

mod bake_files;
mod disk_text;

pub use bake_files::{Action, BakeFiles, Saved};
pub use disk_text::DiskText;

use crate::serialize::SceneData;
use crate::Scene;

/// The one standardised scene file extension (JSON inside).
pub const SCENE_EXTENSION: &str = "scene";

/// File ▸ New and `Scene.New()`, one path (#2000): replace `scene` with an empty
/// one (no entities, default ambient/sky/layers, a new identity) and clear
/// `current`, the current scene file, so nothing writes back over the old file.
/// Refused during Play, as Unity greys File ▸ New out: the swap would skip every
/// script's `OnDisable`/`OnDestroy` (#1305). An unsaved scene is discarded, as a
/// load discards it.
pub fn new_scene(scene: &mut Scene, current: &mut Option<String>) -> Result<(), &'static str> {
    if scene.is_playing() {
        return Err(NEW_IN_PLAY);
    }
    scene.clear();
    *current = None;
    Ok(())
}

/// Why [`new_scene`] refused: the World holds the play state, and Play owns it.
pub const NEW_IN_PLAY: &str = "a new scene cannot be started during Play: stop Play first";

/// Read and parse the scene document at `path` without touching any live scene — so
/// a caller can fail cleanly before tearing the current one down (#432).
pub fn read_scene_file(path: &str) -> Result<SceneData, String> {
    parse_scene_text(&read_text(path)?, path)
}

/// The text of the scene file at `path`, its error naming the path.
pub fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read scene file {}: {}", path, e))
}

/// Parse `json`, the scene document read from `path` (named in the error).
pub fn parse_scene_text(json: &str, path: &str) -> Result<SceneData, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to deserialize scene {}: {}", path, e))
}
