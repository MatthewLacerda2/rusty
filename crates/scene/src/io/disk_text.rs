//! The text the engine last read or wrote at the scene's file (#1340).
//!
//! The editor follows its open scene's file on disk (`shell::editor::watch::follow`):
//! a file that changed outside the engine reloads. "Outside" is decided against this
//! record, the exact text of the last `load_from_file`, `save_to_file` or
//! `save_lighting` at that path (`edit_side::io`). A file holding that text is the engine's own write (a save's echo,
//! Generate Lighting rewriting its fields, #832) or no change at all (a `touch`, a
//! rescan, the same bytes written again), never a reason to reload.
//!
//! Interior mutability because every writer takes `&Scene`; one per scene, never
//! serialized.

use std::cell::RefCell;

use crate::Scene;

/// The last `(path, text)` the engine read or wrote for this scene.
#[derive(Clone, Default)]
pub struct DiskText(RefCell<Option<(String, String)>>);

impl DiskText {
    /// Remember that the file at `path` now holds `text`.
    fn record(&self, path: &str, text: String) {
        *self.0.borrow_mut() = Some((path.to_string(), text));
    }

    /// Whether `text` is what the engine last read or wrote at `path`.
    fn is(&self, path: &str, text: &str) -> bool {
        let last = self.0.borrow();
        last.as_ref().is_some_and(|(p, t)| p == path && t == text)
    }

    fn forget(&self) {
        *self.0.borrow_mut() = None;
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

    /// The record as it stands, for a host that lends the World to another scene
    /// and puts this one back (`dev::demo::Origin`, #1598).
    pub fn disk_text(&self) -> DiskText {
        self.disk_text.clone()
    }

    /// Take back `record`, held by [`disk_text`](Self::disk_text): the scene put back
    /// is the one that read or wrote it.
    pub fn restore_disk_text(&mut self, record: DiskText) {
        self.disk_text = record;
    }

    /// The World was rebuilt to stand for `path` without reading it (Reset Scene,
    /// #1374): record the file's text as it is, whatever it holds, so a `touch`, a
    /// rescan or the editor's own Generate Lighting is no change. A real outside edit
    /// changes the text, so it still reloads, and says the rebuilt scene was unsaved
    /// when it differs from the file. An unreadable file leaves no record.
    pub fn adopt_disk_text(&self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(disk) => self.disk_text.record(path, disk),
            Err(_) => self.disk_text.forget(),
        }
    }

    /// Remember that the scene's file at `path` now holds `text`: what a save
    /// wrote or a load read (`edit_side::io`).
    pub fn record_disk_text(&self, path: &str, text: String) {
        self.disk_text.record(path, text);
    }

    /// The text the engine last read or wrote at `path`, if `path` is the one
    /// recorded: what `has_unsaved_edits` compares the live scene with.
    pub fn disk_text_at(&self, path: &str) -> Option<String> {
        self.disk_text.text(path)
    }
}
