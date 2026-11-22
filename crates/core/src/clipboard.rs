//! The shell captures the OS clipboard text at a boundary; [`ClipboardRecord::begin_tick`]
//! publishes it to the next tick. A game's write is a request the platform applies after
//! the tick, yet is readable at once, so copy and paste agree without an OS clipboard.

#[derive(Clone, Debug, Default)]
pub struct ClipboardRecord {
    pending: Option<String>,
    current: String,
    request: Option<String>,
}

impl ClipboardRecord {
    pub fn capture(&mut self, text: String) {
        self.pending = Some(text);
    }

    pub fn pending(&self) -> Option<&str> {
        self.pending.as_deref()
    }

    /// A demo replay's capture, in place of the platform's.
    pub fn set_pending(&mut self, text: Option<String>) {
        self.pending = text;
    }

    pub fn begin_tick(&mut self) {
        if let Some(text) = self.pending.take() {
            self.current = text;
        }
    }

    pub fn text(&self) -> &str {
        &self.current
    }

    pub fn set(&mut self, text: &str) {
        self.pending = None;
        self.current = text.to_string();
        self.request = Some(text.to_string());
    }

    pub fn take_request(&mut self) -> Option<String> {
        self.request.take()
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardRecord;

    #[test]
    fn a_capture_waits_for_the_next_tick() {
        let mut clip = ClipboardRecord::default();
        clip.capture("os".into());
        assert_eq!(clip.text(), "", "a tick never sees a mid-tick capture");
        clip.begin_tick();
        assert_eq!(clip.text(), "os");
        clip.begin_tick();
        assert_eq!(clip.text(), "os", "the text holds until captured again");
    }

    #[test]
    fn a_write_reads_back_at_once_and_is_requested_once() {
        let mut clip = ClipboardRecord::default();
        clip.set("copied");
        assert_eq!(clip.text(), "copied");
        clip.begin_tick();
        assert_eq!(clip.text(), "copied", "the next tick keeps it");
        assert_eq!(clip.take_request().as_deref(), Some("copied"));
        assert_eq!(clip.take_request(), None);
    }
}
