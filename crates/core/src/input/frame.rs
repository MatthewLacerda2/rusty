//! The input the next tick reads, taken before its `begin_tick`, so a demo replay writes
//! back exactly what was recorded. Sets and maps are sorted so frames compare and encode
//! in one order.

//! The sets and maps are sorted vectors, so two frames compare and encode in one
//! order whatever order the keys were pressed in.

use super::gamepad::MAX_PADS;
use super::{InputState, TickInput};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputFrame {
    pub held: Vec<String>,
    pub down: Vec<String>,
    pub up: Vec<String>,
    pub mouse_delta: (f64, f64),
    pub scroll: f64,
    pub text: String,
    pub axes: Vec<(String, f32)>,
    pub mouse_position: (f64, f64),
    pub pads: [bool; MAX_PADS],
    pub clipboard: Option<String>,
}

fn sorted(set: &crate::collections::Set<String>) -> Vec<String> {
    let mut keys: Vec<String> = set.iter().cloned().collect();
    keys.sort();
    keys
}

impl InputState {
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
