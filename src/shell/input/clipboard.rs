//! src/shell/input/clipboard.rs — the clipboard pump (#612): OS text in, the game's
//! writes out.
//!
//! The sim reads a *captured* copy of the clipboard (`core::clipboard`), so a
//! replay that injects the same text pastes identically. The shell captures only at
//! the boundaries a paste can follow — a key going down with Ctrl or Cmd held (the
//! capture lands before that key's tick), and the window regaining focus (the player
//! copied something in another app) — rather than every frame, because an OS
//! clipboard read is a round trip to the display server. After each tick it copies
//! the game's `Input.SetClipboard` request to the OS.
//!
//! The source is a trait so tests drive the pump with a fake: CI has no clipboard
//! to trust. The real one is [`OsClipboard`](super::clipboard_source::OsClipboard).

use winit::keyboard::ModifiersState;

use crate::core::input::InputState;

/// Where clipboard text comes from and goes to: the OS in a window, a fake in tests.
pub trait ClipboardSource {
    /// The clipboard's text; `None` when it holds none (empty, or an image).
    fn read(&mut self) -> Option<String>;
    /// Put `text` on the clipboard.
    fn write(&mut self, text: &str);
}

/// Whether a key press with these modifiers may be a paste, so the clipboard is
/// captured for the tick it lands on: Ctrl (Linux) or Cmd (macOS) is held.
pub fn is_shortcut(modifiers: ModifiersState) -> bool {
    modifiers.control_key() || modifiers.super_key()
}

/// Snapshot the OS clipboard into the sim; no text reads as `""`.
pub fn capture(source: &mut dyn ClipboardSource, input: &mut InputState) {
    let text = source.read().unwrap_or_default();
    input.clipboard.capture(text);
}

/// Copy the game's pending clipboard write, if any, to the OS.
pub fn apply_request(source: &mut dyn ClipboardSource, input: &mut InputState) {
    if let Some(text) = input.clipboard.take_request() {
        source.write(&text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        text: Option<String>,
        writes: Vec<String>,
    }

    impl ClipboardSource for Fake {
        fn read(&mut self) -> Option<String> {
            self.text.clone()
        }
        fn write(&mut self, text: &str) {
            self.writes.push(text.to_string());
            self.text = Some(text.to_string());
        }
    }

    #[test]
    fn a_capture_reaches_the_next_tick_and_no_text_reads_empty() {
        let (mut os, mut input) = (Fake::default(), InputState::new());
        os.text = Some("10.0.0.7".into());
        capture(&mut os, &mut input);
        input.begin_tick();
        assert_eq!(input.clipboard.text(), "10.0.0.7");
        os.text = None;
        capture(&mut os, &mut input);
        input.begin_tick();
        assert_eq!(input.clipboard.text(), "");
    }

    #[test]
    fn a_game_write_is_applied_to_the_os_once() {
        let (mut os, mut input) = (Fake::default(), InputState::new());
        input.clipboard.set("copied");
        apply_request(&mut os, &mut input);
        apply_request(&mut os, &mut input);
        assert_eq!(os.writes, vec!["copied"]);
        capture(&mut os, &mut input);
        input.begin_tick();
        assert_eq!(input.clipboard.text(), "copied", "the OS reads it back");
    }

    #[test]
    fn only_ctrl_or_cmd_chords_capture() {
        assert!(is_shortcut(ModifiersState::CONTROL));
        assert!(is_shortcut(ModifiersState::SUPER | ModifiersState::SHIFT));
        assert!(!is_shortcut(ModifiersState::SHIFT));
        assert!(!is_shortcut(ModifiersState::empty()));
    }
}
