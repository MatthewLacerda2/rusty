//! Keys, mouse buttons and pad buttons are all named keys in one set; names are
//! uppercase. Writes between ticks accumulate in a pending buffer that
//! [`InputState::begin_tick`] publishes, so a key pressed and released between two ticks
//! still reports both edges. The cursor and clipboard are requests the platform applies.

mod frame;
pub mod gamepad;
pub mod keymap;

pub use frame::InputFrame;

use crate::collections::{Map, Set};

use crate::clipboard::ClipboardRecord;
use crate::input::gamepad::PadRecords;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorState {
    pub locked: bool,
    pub visible: bool,
}

impl CursorState {
    pub const PLAY: Self = Self {
        locked: true,
        visible: false,
    };
    pub const FREE: Self = Self {
        locked: false,
        visible: true,
    };
}

#[derive(Clone, Debug, Default)]
struct TickInput {
    down: Set<String>,
    up: Set<String>,
    mouse_delta: (f64, f64),
    scroll: f64,
    text: String,
}

#[derive(Clone, Debug)]
pub struct InputState {
    keys_down: Set<String>,
    /// Game-view pixels, origin top-left (the platform maps window → game view).
    mouse_position: (f64, f64),
    pending: TickInput,
    current: TickInput,
    cursor: CursorState,
    pending_axes: Map<String, f32>,
    current_axes: Map<String, f32>,
    pub pads: PadRecords,
    pub clipboard: ClipboardRecord,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys_down: Set::default(),
            mouse_position: (0.0, 0.0),
            pending: TickInput::default(),
            current: TickInput::default(),
            cursor: CursorState::FREE,
            pending_axes: Map::default(),
            current_axes: Map::default(),
            pads: PadRecords::default(),
            clipboard: ClipboardRecord::default(),
        }
    }
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_tick(&mut self) {
        self.current = std::mem::take(&mut self.pending);
        self.current_axes.clone_from(&self.pending_axes);
        self.clipboard.begin_tick();
    }

    /// Set a key (or mouse button) up/down. Only a real change records an edge, so
    /// OS key-repeat and a release of a key that was never down are no-ops.
    pub fn set_key_state(&mut self, key_name: &str, pressed: bool) {
        let key = key_name.to_uppercase();
        if pressed {
            if self.keys_down.insert(key.clone()) {
                self.pending.down.insert(key);
            }
        } else if self.keys_down.remove(&key) {
            self.pending.up.insert(key);
        }
    }

    pub fn is_key_down(&self, key_name: &str) -> bool {
        self.keys_down.contains(&key_name.to_uppercase())
    }

    pub fn get_key_down(&self, key_name: &str) -> bool {
        self.current.down.contains(&key_name.to_uppercase())
    }

    pub fn get_key_up(&self, key_name: &str) -> bool {
        self.current.up.contains(&key_name.to_uppercase())
    }

    pub fn press(&mut self, key_name: &str) {
        self.set_key_state(key_name, true);
    }

    pub fn release(&mut self, key_name: &str) {
        self.set_key_state(key_name, false);
    }

    pub fn release_all(&mut self) {
        for key in std::mem::take(&mut self.keys_down) {
            self.pending.up.insert(key);
        }
    }

    pub fn set_axis(&mut self, axis_name: &str, value: f32) {
        let value = if value.is_nan() {
            0.0
        } else {
            value.clamp(-1.0, 1.0)
        };
        self.pending_axes.insert(axis_name.to_uppercase(), value);
    }

    pub fn axis(&self, axis_name: &str) -> f32 {
        let axis = axis_name.to_uppercase();
        self.current_axes.get(&axis).copied().unwrap_or(0.0)
    }

    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    pub fn move_mouse(&mut self, x: f64, y: f64) {
        self.mouse_position = (x, y);
    }

    pub fn add_mouse_delta(&mut self, dx: f64, dy: f64) {
        self.pending.mouse_delta.0 += dx;
        self.pending.mouse_delta.1 += dy;
    }

    pub fn mouse_delta(&self) -> (f64, f64) {
        self.current.mouse_delta
    }

    /// Accumulate vertical wheel motion in lines (positive = away from the user).
    pub fn scroll(&mut self, dy: f64) {
        self.pending.scroll += dy;
    }

    pub fn scroll_delta(&self) -> f64 {
        self.current.scroll
    }

    pub fn type_text(&mut self, text: &str) {
        self.pending.text.push_str(text);
    }

    pub fn text_input(&self) -> &str {
        &self.current.text
    }

    pub fn cursor(&self) -> CursorState {
        self.cursor
    }

    pub fn set_cursor_locked(&mut self, locked: bool) {
        self.cursor.locked = locked;
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor.visible = visible;
    }

    pub fn reset_cursor(&mut self, cursor: CursorState) {
        self.cursor = cursor;
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod input_tests;
