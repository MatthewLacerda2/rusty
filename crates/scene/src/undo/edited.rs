//! Whether an edit landed, for the editor's unsaved mark: a committed or folded step, an Undo or
//! Redo, and in Play, which keeps no step, a recorded write that changed an entity's saved form.
//! Play's writes are weighed at each `pause_edits`, before the sim moves what they touched.

use std::cell::Cell;

use crate::Scene;

/// A full save (`rusty_edit::save_to_file`, any caller) since the editor looked, and whether
/// an edit landed after it; a `Cell`, as the save takes `&Scene`.
#[derive(Debug, Default)]
pub struct Saves(Cell<Option<bool>>);

impl Scene {
    /// Since the last call; a write that changes nothing saved is no edit.
    pub fn take_edited(&mut self) -> bool {
        std::mem::take(&mut self.history.edited)
    }

    /// An unrecorded write only a full save keeps: an edit.
    pub fn mark_unsaved(&mut self) {
        self.note_edit();
    }

    pub fn note_saved(&self) {
        self.saves.0.set(Some(false));
    }

    pub fn take_saved(&mut self) -> Option<bool> {
        self.saves.0.take()
    }

    pub(super) fn note_edit(&mut self) {
        self.history.edited = true;
        if self.saves.0.get().is_some() {
            self.saves.0.set(Some(true));
        }
    }
}
