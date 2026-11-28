//! The text the engine last read or wrote at each loaded scene's file. The editor's follow
//! (`shell::editor::watch`) reloads a file changed outside the engine; one holding this
//! text is the engine's own write or no change. Interior mutability: writers take `&Scene`.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::Scene;

#[derive(Clone, Default)]
pub struct DiskText {
    last: RefCell<BTreeMap<String, String>>,
    pub(crate) saves: crate::undo::Saves,
}

impl DiskText {
    /// The World now stands for `path` alone.
    fn record(&self, path: &str, text: String) {
        let mut last = self.last.borrow_mut();
        last.clear();
        last.insert(path.to_string(), text);
    }

    fn record_also(&self, path: &str, text: String) {
        self.last.borrow_mut().insert(path.to_string(), text);
    }

    fn is(&self, path: &str, text: &str) -> bool {
        self.last.borrow().get(path).is_some_and(|t| t == text)
    }

    fn forget(&self) {
        self.last.borrow_mut().clear();
    }

    fn text(&self, path: &str) -> Option<String> {
        self.last.borrow().get(path).cloned()
    }
}

impl Scene {
    pub fn is_disk_text(&self, path: &str, text: &str) -> bool {
        self.disk_text.is(path, text)
    }

    pub fn disk_text(&self) -> DiskText {
        self.disk_text.clone()
    }

    pub fn restore_disk_text(&mut self, record: DiskText) {
        self.disk_text = record;
    }

    /// The World was rebuilt to stand for `path` unread (Reset Scene): its text as it is, so a
    /// `touch` or the editor's own Generate Lighting is no change.
    pub fn adopt_disk_text(&self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(disk) => self.disk_text.record(path, disk),
            Err(_) => self.disk_text.forget(),
        }
    }

    pub fn record_disk_text(&self, path: &str, text: String) {
        self.disk_text.record(path, text);
    }

    /// One loaded scene's file among several: the others' records stay.
    pub fn record_disk_text_also(&self, path: &str, text: String) {
        self.disk_text.record_also(path, text);
    }

    pub fn forget_disk_text_at(&self, path: &str) {
        self.disk_text.last.borrow_mut().remove(path);
    }

    pub fn disk_text_at(&self, path: &str) -> Option<String> {
        self.disk_text.text(path)
    }
}
