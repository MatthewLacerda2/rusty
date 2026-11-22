//! crates/ui/src/events/focus/actions.rs — input → logical navigation actions.

use glam::Vec2;

use super::DIRECTIONS;
use rusty_core::input::gamepad::{pad_name, MAX_PADS};
use rusty_core::input::InputState;

/// A logical navigation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction {
    Move(usize),
    Next,
    Previous,
    Submit,
    Cancel,
}

/// Seconds a held Move waits before its first repeat (Unity's `moveRepeatDelay`).
pub const REPEAT_DELAY: f32 = 0.5;
/// Seconds between later repeats (Unity's `moveRepeatRate`).
pub const REPEAT_RATE: f32 = 0.1;
const STICK_PRESS: f32 = 0.5;

/// The keys whose press is a Move, direction-major in `select_on` order: each
/// direction's arrow, then every pad slot's d-pad button.
fn move_keys() -> impl Iterator<Item = (usize, String)> {
    let dirs = ["UP", "DOWN", "LEFT", "RIGHT"];
    dirs.into_iter().enumerate().flat_map(|(i, d)| {
        let pads = (0..MAX_PADS).map(move |pad| pad_name(pad, d));
        std::iter::once(d.to_string())
            .chain(pads)
            .map(move |k| (i, k))
    })
}

/// Whether `suffix` (a pad button) went down on any pad slot this tick.
fn any_pad_down(input: &InputState, suffix: &str) -> bool {
    (0..MAX_PADS).any(|pad| input.get_key_down(&pad_name(pad, suffix)))
}

pub fn nav_actions(input: &InputState) -> Vec<NavAction> {
    let mut actions: Vec<NavAction> = Vec::new();
    for (dir, key) in move_keys() {
        if input.get_key_down(&key) && !actions.contains(&NavAction::Move(dir)) {
            actions.push(NavAction::Move(dir));
        }
    }
    if input.get_key_down("TAB") {
        let shift = input.is_key_down("LEFTSHIFT") || input.is_key_down("RIGHTSHIFT");
        actions.push(if shift {
            NavAction::Previous
        } else {
            NavAction::Next
        });
    }
    if input.get_key_down("ENTER") || input.get_key_down("KEYPADENTER") || any_pad_down(input, "A")
    {
        actions.push(NavAction::Submit);
    }
    if input.get_key_down("ESCAPE") || any_pad_down(input, "B") {
        actions.push(NavAction::Cancel);
    }
    actions
}

/// The one direction everything held points (`select_on` order): held arrows and
/// d-pads, plus each stick past [`STICK_PRESS`], summed; the dominant axis wins,
/// vertical on a tie (Unity's `MoveDirection`).
pub fn held_direction(input: &InputState) -> Option<usize> {
    let mut v = Vec2::ZERO;
    for (dir, key) in move_keys() {
        if input.is_key_down(&key) {
            v += DIRECTIONS[dir];
        }
    }
    for pad in 0..MAX_PADS {
        let x = input.axis(&pad_name(pad, "LEFTX"));
        let y = input.axis(&pad_name(pad, "LEFTY"));
        let stick = Vec2::new(x, y);
        if stick.length() >= STICK_PRESS {
            v += stick;
        }
    }
    if v.x.abs() > v.y.abs() {
        Some(if v.x < 0.0 { 2 } else { 3 })
    } else if v.y != 0.0 {
        Some(if v.y > 0.0 { 0 } else { 1 })
    } else {
        None
    }
}

const TICK_SLACK: f32 = 1e-4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveRepeat {
    dir: Option<usize>,
    held: f32,
    next: f32,
}

impl MoveRepeat {
    /// This tick's Moves from the held direction after `dt` unscaled seconds.
    pub fn tick(&mut self, held: Option<usize>, pressed: bool, dt: f32) -> Option<NavAction> {
        if pressed || held != self.dir {
            *self = Self {
                dir: held,
                held: 0.0,
                next: REPEAT_DELAY,
            };
            return held.filter(|_| !pressed).map(NavAction::Move);
        }
        let dir = self.dir?;
        self.held += dt;
        if self.held + TICK_SLACK < self.next {
            return None;
        }
        self.next = self.held + REPEAT_RATE;
        Some(NavAction::Move(dir))
    }
}
