//! src/core/clipboard.rs — the clipboard as the sim sees it (#612).
//!
//! The OS clipboard is platform I/O, so the sim never reads it directly: the shell
//! **captures** its text into this record at a boundary (a key going down with Ctrl
//! or Cmd held, and the window regaining focus), and [`ClipboardRecord::begin_tick`]
//! publishes the capture to the next tick, like an axis. A game's write
//! (`Input.SetClipboard`) is a *request* the platform applies after the tick, like
//! the cursor request, and it updates what the sim reads at once, so a copy and a
//! paste in the same run agree with or without an OS clipboard. Bots and the harness
//! set the clipboard the same way a game does, so a paste replays identically.

/// The clipboard text the sim reads, and the write it asked the platform for.
#[derive(Clone, Debug, Default)]
pub struct ClipboardRecord {
    /// The capture since the last tick, if any; published by [`Self::begin_tick`]
    /// and gone with it, so the text is not carried (and copied) tick after tick.
    pending: Option<String>,
    /// What this tick reads.
    current: String,
    /// The game's last write, not yet applied to the OS clipboard.
    request: Option<String>,
}

impl ClipboardRecord {
    /// The platform's snapshot of the OS clipboard text; published on the next tick.
    pub fn capture(&mut self, text: String) {
        self.pending = Some(text);
    }

    /// The capture the next tick will publish, if one was made since the last.
    pub fn pending(&self) -> Option<&str> {
        self.pending.as_deref()
    }

    /// Replace the capture the next tick publishes: a demo replay's (#1581).
    pub fn set_pending(&mut self, text: Option<String>) {
        self.pending = text;
    }

    /// Publish the latest capture as this tick's clipboard; without one, the text
    /// holds.
    pub fn begin_tick(&mut self) {
        if let Some(text) = self.pending.take() {
            self.current = text;
        }
    }

    /// The clipboard text as of this tick.
    pub fn text(&self) -> &str {
        &self.current
    }

    /// The game (or a bot) sets the clipboard: readable at once, and recorded for the
    /// platform to copy to the OS clipboard after the tick.
    pub fn set(&mut self, text: &str) {
        self.pending = None;
        self.current = text.to_string();
        self.request = Some(text.to_string());
    }

    /// The write the platform still owes the OS clipboard, if any.
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
