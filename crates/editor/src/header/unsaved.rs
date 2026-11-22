//! The unsaved mark (`EditorUi::is_dirty`), which a save clears: raised in one place, from what
//! the Undo history weighed (`Scene::take_edited`), so no inspector card, menu, gizmo, console
//! line or MCP call sets it by hand, and a write that changes nothing saved leaves it alone. A
//! write outside the history (a bake applied, a model's instances rebuilt) marks it itself.

use crate::EditorUi;
use rusty_scene::Scene;

impl EditorUi {
    /// Whether an edit landed since the last call.
    pub fn follow_edits(&mut self, scene: &mut Scene) -> bool {
        let edited = scene.take_edited();
        self.is_dirty |= edited;
        edited
    }
}
