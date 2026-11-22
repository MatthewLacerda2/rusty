use glam::Vec2;

use super::DIRECTIONS;
use rusty_core::input::gamepad::{pad_name, MAX_PADS};
use rusty_core::input::InputState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction {
    /// `0` up, `1` down, `2` left, `3` right (`select_on` order).
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
/// Stick magnitude (0–1) that counts as a Move.
const STICK_PRESS: f32 = 0.5;

/// Each direction's arrow, then every pad slot's d-pad button, direction-major.
fn move_keys() -> impl Iterator<Item = (usize, String)> {
    let dirs = ["UP", "DOWN", "LEFT", "RIGHT"];
    dirs.into_iter().enumerate().flat_map(|(i, d)| {
        let pads = (0..MAX_PADS).map(move |pad| pad_name(pad, d));
        std::iter::once(d.to_string())
            .chain(pads)
            .map(move |k| (i, k))
    })
}

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

/// The one direction held keys and sticks past [`STICK_PRESS`] sum to (Unity's `MoveDirection`).
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

/// Seconds of slack for f32 rounding in summed dts: at 1/60 s a 0.1 s repeat lands every
/// 6th tick, not 7th.
const TICK_SLACK: f32 = 1e-4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveRepeat {
    dir: Option<usize>,
    held: f32,
    next: f32,
}

impl MoveRepeat {
    /// The held Move's repeat after `dt` unscaled seconds; a `pressed` edge (already sent)
    /// restarts it.
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
