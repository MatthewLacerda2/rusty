//! src/shell/editor/last_scene.rs — the editor remembers the scene it has open
//! (#1618), so the next launch reopens it (`core::project::boot_scene`), as Unity
//! reopens the last scene from `Library/`. Only the edit scene counts: a scene a
//! game loads in Play is the game's, and Stop brings the edit scene back. An
//! Untitled scene, or one outside the project, leaves the record alone
//! (`core::project::scene_to_remember` decides).

use super::EditorFrontend;
use crate::app::GameWorld;

impl EditorFrontend {
    /// Write the open scene's path to the project's record when it changed: Open,
    /// Save As, a scripted `Scene.Load` in edit mode. One string compare a frame;
    /// the record is worked out only on a change.
    pub(super) fn remember_open_scene(&mut self, game: &GameWorld) {
        let playing = game.scene().borrow().is_playing();
        let last = self.last_scene.as_deref();
        let changed = self.editor_ui.with_current_scene_path(|open| {
            (!playing && open != last).then(|| open.map(str::to_string))
        });
        let Some(open) = changed else {
            return;
        };
        // Moved on even when nothing is written (Untitled, outside the project, a
        // failed write), so a read-only `cache/` warns once, not every frame.
        let previous = std::mem::replace(&mut self.last_scene, open);
        let root = crate::core::project::root();
        let (open, last) = (self.last_scene.as_deref(), previous.as_deref());
        let Some(record) = crate::core::project::scene_to_remember(&root, open, last, playing)
        else {
            return;
        };
        if let Err(err) = crate::core::project::remember_scene(&root, &record) {
            log::warn!("[Project] could not remember the open scene: {err}");
        }
    }
}
