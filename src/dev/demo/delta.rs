//! A tick's input as the change from the tick before (#1581).
//!
//! Most ticks repeat the last one's held keys and pointer and carry no edge, so
//! a demo writes only the fields of an [`InputFrame`] that changed: an hour of
//! play is mostly empty ticks, which it does not write at all.

use serde::{Deserialize, Serialize};

use super::format::Exact;
use crate::core::input::gamepad::MAX_PADS;
use crate::core::input::InputFrame;

/// The fields of a frame that differ from the frame before; `None` is unchanged.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    held: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    down: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    up: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_delta: Option<[Exact; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scroll: Option<Exact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    axes: Option<Vec<(String, Exact)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_position: Option<[Exact; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pads: Option<[bool; MAX_PADS]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clipboard: Option<String>,
}

/// `Some(now)` when it differs from `before`.
fn changed<T: PartialEq + Clone>(before: &T, now: &T) -> Option<T> {
    (before != now).then(|| now.clone())
}

fn pair(p: (f64, f64)) -> [Exact; 2] {
    [Exact(p.0), Exact(p.1)]
}

impl InputDelta {
    /// What changed from `before` to `now`.
    pub fn between(before: &InputFrame, now: &InputFrame) -> Self {
        let axes = changed(&before.axes, &now.axes).map(|axes| {
            axes.into_iter()
                .map(|(name, value)| (name, Exact(f64::from(value))))
                .collect()
        });
        Self {
            held: changed(&before.held, &now.held),
            down: changed(&before.down, &now.down),
            up: changed(&before.up, &now.up),
            mouse_delta: changed(&before.mouse_delta, &now.mouse_delta).map(pair),
            scroll: changed(&before.scroll.to_bits(), &now.scroll.to_bits())
                .map(|_| Exact(now.scroll)),
            text: changed(&before.text, &now.text),
            axes,
            mouse_position: changed(&before.mouse_position, &now.mouse_position).map(pair),
            pads: changed(&before.pads, &now.pads),
            clipboard: changed(&before.clipboard, &now.clipboard),
        }
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The frame after `before` this delta describes.
    pub fn apply(&self, before: &InputFrame) -> InputFrame {
        let mut now = before.clone();
        let set = |slot: &mut Vec<String>, value: &Option<Vec<String>>| {
            if let Some(value) = value {
                slot.clone_from(value);
            }
        };
        set(&mut now.held, &self.held);
        set(&mut now.down, &self.down);
        set(&mut now.up, &self.up);
        if let Some([x, y]) = self.mouse_delta {
            now.mouse_delta = (x.0, y.0);
        }
        if let Some(scroll) = self.scroll {
            now.scroll = scroll.0;
        }
        if let Some(text) = &self.text {
            now.text.clone_from(text);
        }
        if let Some(axes) = &self.axes {
            // An `f32` widened to `f64` narrows back exactly.
            now.axes = axes.iter().map(|(n, v)| (n.clone(), v.0 as f32)).collect();
        }
        if let Some([x, y]) = self.mouse_position {
            now.mouse_position = (x.0, y.0);
        }
        if let Some(pads) = self.pads {
            now.pads = pads;
        }
        if let Some(clipboard) = &self.clipboard {
            now.clipboard.clone_from(clipboard);
        }
        now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delta_carries_only_what_changed_and_rebuilds_the_frame() {
        let before = InputFrame {
            held: vec!["W".into()],
            mouse_position: (3.0, 4.0),
            ..InputFrame::default()
        };
        let now = InputFrame {
            held: vec!["W".into()],
            down: vec!["MOUSE0".into()],
            mouse_delta: (0.1, -1.0 / 3.0),
            axes: vec![("PADLEFTX".into(), 0.3)],
            mouse_position: (3.0, 4.0),
            ..InputFrame::default()
        };
        let delta = InputDelta::between(&before, &now);
        let text = serde_json::to_string(&delta).unwrap();
        assert!(!text.contains("held") && !text.contains("mouse_position"));
        let read: InputDelta = serde_json::from_str(&text).unwrap();
        assert_eq!(read.apply(&before), now, "bit for bit through the file");
        assert!(InputDelta::between(&now, &now).is_empty());
    }
}
