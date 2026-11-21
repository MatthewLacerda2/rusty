use std::cell::RefCell;

use crate::Scene;

#[derive(Clone, Default)]
pub struct DiskText(RefCell<Option<(String, String)>>);

impl DiskText {
    fn record(&self, path: &str, text: String) {
        *self.0.borrow_mut() = Some((path.to_string(), text));
    }

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
    pub fn is_disk_text(&self, path: &str, text: &str) -> bool {
        self.disk_text.is(path, text)
    }

    pub fn disk_text(&self) -> DiskText {
        self.disk_text.clone()
    }

    pub fn restore_disk_text(&mut self, record: DiskText) {
        self.disk_text = record;
    }

    pub fn adopt_disk_text(&self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(disk) => self.disk_text.record(path, disk),
            Err(_) => self.disk_text.forget(),
        }
    }

    pub fn record_disk_text(&self, path: &str, text: String) {
        self.disk_text.record(path, text);
    }

    pub fn disk_text_at(&self, path: &str) -> Option<String> {
        self.disk_text.text(path)
    }
}
