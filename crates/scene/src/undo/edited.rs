//! Whether an edit landed, for the editor's unsaved mark: a committed or folded step, an Undo or
//! Redo, and in Play, which keeps no step, a recorded write that changed an entity's saved form.
//! Play's writes are weighed at each `pause_edits`, before the sim moves what they touched.

use crate::Scene;

impl Scene {
    /// Since the last call; a write that changes nothing saved is no edit.
    pub fn take_edited(&mut self) -> bool {
        std::mem::take(&mut self.history.edited)
    }

    /// An unrecorded write only a full save keeps (a bake applied, not written): an edit.
    pub fn mark_unsaved(&mut self) {
        self.history.edited = true;
    }
}
