//! The unsaved mark (`EditorUi::is_dirty`), from the Undo history's edits. Only a full save
//! clears it (`Scene::take_saved`), never a comparison with the file: it also stands for
//! what is outside the document (a bake's probe SH).

use crate::EditorUi;
use rusty_scene::Scene;

impl EditorUi {
    /// Whether an edit landed since the last call, in Play too (a demo it taints).
    pub fn follow_edits(&mut self, scene: &mut Scene) -> bool {
        let edited = scene.take_edited();
        self.is_dirty |= edited && !scene.is_playing();
        if let Some(edited_after) = scene.take_saved() {
            self.is_dirty = edited_after;
        }
        edited
    }

    /// The scene was replaced: the old one's notes are spent.
    pub fn loaded(&mut self, scene: &mut Scene) {
        scene.take_edited();
        scene.take_saved();
        self.is_dirty = false;
    }

    /// In Play the frame's own fold keeps the edit, for the demo's taint.
    pub fn unsaved(&mut self, scene: &mut Scene) -> bool {
        if !scene.is_playing() {
            self.follow_edits(scene);
        }
        self.is_dirty
    }
}

#[cfg(test)]
#[path = "unsaved_tests.rs"]
mod tests;
