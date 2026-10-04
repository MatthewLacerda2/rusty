//! src/core/input.rs — `InputState`, the sim's one input resource.
//!
//! Frame input as a swappable *source*: the simulation reads this, but it does not
//! care who wrote it — winit (windowed play, via `shell::input`), the headless
//! harness, or a bot-player script all drive the same state through the same
//! writable API.
//!
//! **Everything is a named key.** Keyboard keys, mouse buttons (`"MOUSE0"` left,
//! `"MOUSE1"` right, `"MOUSE2"` middle, …) and gamepad buttons (`"PADA"`, #471) are
//! entries in one keys-down set, so `IsKeyDown` / `GetKeyDown` / `Press` work on all
//! of them unchanged. Continuous inputs are *axes*: the mouse delta and the wheel,
//! and named axes (`"PADLEFTX"`, see [`gamepad`](crate::core::gamepad)) whose value
//! is sampled per tick like an edge. Names are normalized to uppercase.
//!
//! **Edges are per sim tick.** Writes between ticks (window events, `Press`,
//! `AddMouseDelta`, …) accumulate in a *pending* buffer; [`InputState::begin_tick`]
//! (called once at the top of every `GameWorld::tick`) publishes them as the tick's
//! edges, deltas and text. So a key pressed and released between two ticks still
//! reports both edges on the next one — nothing is lost between frames, and what a
//! tick sees is a pure function of what was written before it.
//!
//! **The cursor is a request.** `SetCursorLocked`/`SetCursorVisible` only record
//! [`CursorState`] here; the platform layer applies it to the OS cursor. The
//! clipboard works the same way ([`clipboard`](crate::core::clipboard), #612).

use crate::core::collections::{Map, Set};

use crate::core::clipboard::ClipboardRecord;
use crate::core::gamepad::PadRecords;

/// What the game asks the platform to do with the OS cursor. The sim only records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorState {
    /// Lock (capture) the cursor to the window — FPS mouse-look.
    pub locked: bool,
    pub visible: bool,
}

impl CursorState {
    /// The default on entering Play: locked and hidden. A script may override it
    /// (a menu unlocks and shows the cursor).
    pub const PLAY: Self = Self {
        locked: true,
        visible: false,
    };
    /// Outside Play: a free, visible cursor.
    pub const FREE: Self = Self {
        locked: false,
        visible: true,
    };
}

/// The per-tick half of input: edges, deltas and typed text. Accumulated in
/// `pending` between ticks, published as `current` by [`InputState::begin_tick`].
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
    /// Named axes as last written; copied into `current_axes` at the tick boundary.
    pending_axes: Map<String, f32>,
    current_axes: Map<String, f32>,
    /// Connected pads, dead zones and rumble requests (#471).
    pub pads: PadRecords,
    /// The clipboard text captured at a boundary, and the game's write (#612).
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

    /// Publish everything written since the last tick as this tick's edges, deltas
    /// and text, and start a fresh pending buffer. Once per sim tick, before any
    /// system runs.
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

    /// Whether the key is held right now.
    pub fn is_key_down(&self, key_name: &str) -> bool {
        self.keys_down.contains(&key_name.to_uppercase())
    }

    /// Whether the key went down since the previous tick (true for this tick only).
    pub fn get_key_down(&self, key_name: &str) -> bool {
        self.current.down.contains(&key_name.to_uppercase())
    }

    /// Whether the key went up since the previous tick (true for this tick only).
    pub fn get_key_up(&self, key_name: &str) -> bool {
        self.current.up.contains(&key_name.to_uppercase())
    }

    /// Press a key (for scripted/bot drivers that don't go through winit).
    pub fn press(&mut self, key_name: &str) {
        self.set_key_state(key_name, true);
    }

    /// Release a key (for scripted/bot drivers that don't go through winit).
    pub fn release(&mut self, key_name: &str) {
        self.set_key_state(key_name, false);
    }

    /// Release every held key (with its up edge) — the platform calls this when the
    /// game loses input mid-hold (window unfocused, editor Game view left), so no key
    /// stays stuck down.
    pub fn release_all(&mut self) {
        for key in std::mem::take(&mut self.keys_down) {
            self.pending.up.insert(key);
        }
    }

    /// Set a named axis (a pad stick or trigger, or any name a bot invents), clamped
    /// to `-1..1`. Unlike a key it is a level, not an edge: the value holds until
    /// written again, and a tick sees what was last written before it.
    pub fn set_axis(&mut self, axis_name: &str, value: f32) {
        let value = if value.is_nan() {
            0.0
        } else {
            value.clamp(-1.0, 1.0)
        };
        self.pending_axes.insert(axis_name.to_uppercase(), value);
    }

    /// A named axis as sampled at the start of this tick; `0` if never written.
    pub fn axis(&self, axis_name: &str) -> f32 {
        let axis = axis_name.to_uppercase();
        self.current_axes.get(&axis).copied().unwrap_or(0.0)
    }

    /// The pointer in game-view pixels, origin top-left.
    pub fn mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }

    /// Move the pointer (game-view pixels). Takes effect immediately, like a key.
    pub fn move_mouse(&mut self, x: f64, y: f64) {
        self.mouse_position = (x, y);
    }

    /// Accumulate raw mouse motion; published on the next tick.
    pub fn add_mouse_delta(&mut self, dx: f64, dy: f64) {
        self.pending.mouse_delta.0 += dx;
        self.pending.mouse_delta.1 += dy;
    }

    /// Raw mouse motion accumulated over the frames before this tick.
    pub fn mouse_delta(&self) -> (f64, f64) {
        self.current.mouse_delta
    }

    /// Accumulate vertical wheel motion in lines (positive = away from the user).
    pub fn scroll(&mut self, dy: f64) {
        self.pending.scroll += dy;
    }

    /// Wheel lines scrolled before this tick.
    pub fn scroll_delta(&self) -> f64 {
        self.current.scroll
    }

    /// Append typed characters; published on the next tick.
    pub fn type_text(&mut self, text: &str) {
        self.pending.text.push_str(text);
    }

    /// The characters typed before this tick, in order.
    pub fn text_input(&self) -> &str {
        &self.current.text
    }

    /// The cursor the game currently requests.
    pub fn cursor(&self) -> CursorState {
        self.cursor
    }

    pub fn set_cursor_locked(&mut self, locked: bool) {
        self.cursor.locked = locked;
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor.visible = visible;
    }

    /// Reset the cursor request to a play-state default ([`CursorState::PLAY`] on
    /// enter, [`CursorState::FREE`] on exit). Runs before `Start`, so a script can
    /// override the default.
    pub fn reset_cursor(&mut self, cursor: CursorState) {
        self.cursor = cursor;
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod input_tests;
