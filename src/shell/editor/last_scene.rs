//! src/shell/editor/last_scene.rs — the editor remembers the scene it has open
//! (#1618), so the next launch reopens it (`core::project::boot_scene`), as Unity
//! reopens the last scene from `Library/`. Only the edit scene counts: a scene a
//! game loads in Play is the game's, and Stop brings the edit scene back. An
//! Untitled scene leaves the record alone.

use super::EditorFrontend;
use crate::app::GameWorld;

impl EditorFrontend {
    /// Write the open scene's path to the project's record when it changed: Open,
    /// Save As, a scripted `Scene.Load` in edit mode. One string compare a frame.
    pub(super) fn remember_open_scene(&mut self, game: &GameWorld) {
        if game.scene().borrow().is_playing() {
            return;
        }
        let open = self.editor_ui.current_scene_path();
        let Some(path) = open.filter(|path| self.last_scene.as_ref() != Some(path)) else {
            return;
        };
        let root = crate::core::project::root();
        if let Err(err) = crate::core::project::remember_scene(&root, &path) {
            log::warn!("[Project] could not remember the open scene: {err}");
        }
        // Set even on a failed write, so a read-only `cache/` warns once, not a frame.
        self.last_scene = Some(path);
    }
}
