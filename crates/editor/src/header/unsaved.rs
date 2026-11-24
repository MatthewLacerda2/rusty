//! The unsaved mark (`EditorUi::is_dirty`), which a save clears: raised in one place, from what the
//! Undo history weighed (`Scene::take_edited`), so nothing sets it by hand, and a write that
//! changes nothing saved leaves it alone. A write outside the history (a bake applied, a model's
//! instances rebuilt) says so on the scene (`Scene::mark_unsaved`). An edit in Play is no unsaved
//! change: Stop throws it away.

use crate::EditorUi;
use rusty_scene::Scene;

impl EditorUi {
    /// Whether an edit landed since the last call, in Play too (a demo it taints).
    pub fn follow_edits(&mut self, scene: &mut Scene) -> bool {
        let edited = scene.take_edited();
        self.is_dirty |= edited && !scene.is_playing();
        edited
    }

    /// Whether the file lacks the open scene's edits; a script's `Scene.Save` clears the mark.
    pub fn unsaved_against_file(&mut self, scene: &mut Scene) -> bool {
        self.follow_edits(scene);
        let path = self.current_scene_path();
        let saved = |p: &String| !rusty_edit::has_unsaved_edits(scene, p);
        if self.is_dirty && path.as_ref().is_some_and(saved) {
            self.is_dirty = false;
        }
        self.is_dirty
    }
}
