//! The text the engine last read or wrote at the scene's file (#1340).
//!
//! The editor follows its open scene's file on disk (`shell::editor::watch::follow`):
//! a file that changed outside the engine reloads. "Outside" is decided against this
//! record, the exact text of the last [`load_from_file`](super::load_from_file),
//! [`save_to_file`](super::save_to_file) or [`save_lighting`](super::save_lighting)
//! at that path. A file holding that text is the engine's own write (a save's echo,
//! Generate Lighting rewriting its fields, #832) or no change at all (a `touch`, a
//! rescan, the same bytes written again), never a reason to reload.
//!
//! Interior mutability because every writer takes `&Scene`; one per scene, never
//! serialized.

use std::cell::RefCell;

use super::read_scene_text;
use crate::scene::Scene;

/// The last `(path, text)` the engine read or wrote for this scene.
#[derive(Default)]
pub struct DiskText(RefCell<Option<(String, String)>>);

impl DiskText {
    /// Remember that the file at `path` now holds `text`.
    pub(crate) fn record(&self, path: &str, text: String) {
        *self.0.borrow_mut() = Some((path.to_string(), text));
    }

    /// Whether `text` is what the engine last read or wrote at `path`.
    fn is(&self, path: &str, text: &str) -> bool {
        let last = self.0.borrow();
        last.as_ref().is_some_and(|(p, t)| p == path && t == text)
    }

    fn text(&self, path: &str) -> Option<String> {
        let last = self.0.borrow();
        last.as_ref()
            .filter(|(p, _)| p == path)
            .map(|(_, t)| t.clone())
    }
}

impl Scene {
    /// Whether `text` is what the engine last read or wrote at `path`: a file holding
    /// it did not change outside the engine.
    pub fn is_disk_text(&self, path: &str, text: &str) -> bool {
        self.disk_text.is(path, text)
    }

    /// Whether the live scene has edits the document last read or written at `path`
    /// lacks. Computed, never a flag: both sides are read as a `SceneData` and
    /// compared without the selection, which the editor copies into the scene every
    /// frame and is not an edit. No record for `path` counts as unsaved.
    pub fn has_unsaved_edits(&self, path: &str) -> bool {
        let Some(on_disk) = self.disk_text.text(path) else {
            return true;
        };
        let live = super::scene_json(self);
        let canon = |text: &str| {
            let mut data = read_scene_text(text, path).ok()?;
            data.selected_entity_id = None;
            serde_json::to_string(&data).ok()
        };
        live.ok().as_deref().and_then(canon) != canon(&on_disk)
    }
}

#[cfg(test)]
#[path = "disk_text_tests.rs"]
mod tests;
