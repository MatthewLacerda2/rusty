//! src/core/input/frame.rs — everything the next tick will publish, as one value (#1581).
//!
//! [`InputFrame`] is the input a tick reads, taken *before* the tick's
//! [`InputState::begin_tick`]: the held keys, the pending edges, deltas and text,
//! the axes, the pointer, the connected pads and the clipboard capture. A demo
//! recorder reads one per tick ([`InputState::frame`]) and a replay writes it back
//! ([`InputState::set_frame`]) at the same point, so the tick publishes exactly
//! what it published when recorded, whoever wrote the input then.
//!
//! The sets and maps are sorted vectors, so two frames compare and encode in one
//! order whatever order the keys were pressed in.

use super::gamepad::MAX_PADS;
use super::{InputState, TickInput};

/// The input one tick reads, before the tick publishes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputFrame {
    /// Held keys, mouse buttons and pad buttons, sorted.
    pub held: Vec<String>,
    /// Keys that went down since the last tick, sorted.
    pub down: Vec<String>,
    /// Keys that went up since the last tick, sorted.
    pub up: Vec<String>,
    pub mouse_delta: (f64, f64),
    pub scroll: f64,
    pub text: String,
    /// Named axes as last written, sorted by name.
    pub axes: Vec<(String, f32)>,
    pub mouse_position: (f64, f64),
    pub pads: [bool; MAX_PADS],
    /// The clipboard text captured since the last tick, if any.
    pub clipboard: Option<String>,
}

fn sorted(set: &crate::core::collections::Set<String>) -> Vec<String> {
    let mut keys: Vec<String> = set.iter().cloned().collect();
    keys.sort();
    keys
}

impl InputState {
    /// What the next tick will publish, as one value: call it right before the
    /// tick's `begin_tick`.
    pub fn frame(&self) -> InputFrame {
        let mut axes: Vec<(String, f32)> = self
            .pending_axes
            .iter()
            .map(|(name, value)| (name.clone(), *value))
            .collect();
        axes.sort_by(|a, b| a.0.cmp(&b.0));
        InputFrame {
            held: sorted(&self.keys_down),
            down: sorted(&self.pending.down),
            up: sorted(&self.pending.up),
            mouse_delta: self.pending.mouse_delta,
            scroll: self.pending.scroll,
            text: self.pending.text.clone(),
            axes,
            mouse_position: self.mouse_position,
            pads: std::array::from_fn(|pad| self.pads.is_connected(pad)),
            clipboard: self.clipboard.pending().map(str::to_string),
        }
    }

    /// Replace whatever was written since the last tick with `frame`, so the next
    /// `begin_tick` publishes exactly it: the replay half of [`frame`](Self::frame).
    pub fn set_frame(&mut self, frame: &InputFrame) {
        self.keys_down = frame.held.iter().cloned().collect();
        self.pending = TickInput {
            down: frame.down.iter().cloned().collect(),
            up: frame.up.iter().cloned().collect(),
            mouse_delta: frame.mouse_delta,
            scroll: frame.scroll,
            text: frame.text.clone(),
        };
        self.pending_axes = frame.axes.iter().cloned().collect();
        self.mouse_position = frame.mouse_position;
        for (pad, connected) in frame.pads.iter().enumerate() {
            self.pads.set_connected(pad, *connected);
        }
        self.clipboard.set_pending(frame.clipboard.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_written_back_publishes_the_same_tick() {
        let mut live = InputState::new();
        live.press("w");
        live.press("mouse0");
        live.release("mouse0");
        live.add_mouse_delta(1.5, -2.25);
        live.scroll(1.0);
        live.type_text("hi");
        live.set_axis("padleftx", 0.5);
        live.move_mouse(10.0, 20.0);
        live.pads.set_connected(1, true);
        live.clipboard.capture("copied".into());
        let frame = live.frame();

        let mut replay = InputState::new();
        replay.press("q"); // overwritten: a replay ignores live input
        replay.set_frame(&frame);
        assert_eq!(replay.frame(), frame);
        live.begin_tick();
        replay.begin_tick();
        assert!(replay.get_key_down("MOUSE0") && replay.get_key_up("MOUSE0"));
        assert!(replay.is_key_down("W") && !replay.is_key_down("Q"));
        assert_eq!(replay.mouse_delta(), live.mouse_delta());
        assert_eq!(replay.text_input(), "hi");
        assert_eq!(replay.axis("PADLEFTX"), 0.5);
        assert!(replay.pads.is_connected(1));
        assert_eq!(replay.clipboard.text(), "copied");
    }

    #[test]
    fn a_frame_lists_keys_in_one_order() {
        let mut a = InputState::new();
        a.press("b");
        a.press("a");
        let mut b = InputState::new();
        b.press("a");
        b.press("b");
        assert_eq!(a.frame(), b.frame());
        assert_eq!(a.frame().held, ["A", "B"]);
    }
}
