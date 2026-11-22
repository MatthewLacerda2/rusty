mod focus;
mod pointer;
pub mod raycast;
mod transition;
mod tree;

use std::collections::BTreeMap;

use glam::{Vec2, Vec3};

use crate::{UiLayout, UiPointer, UiView};
use rusty_components::SelectionState;
use rusty_core::input::InputState;
use rusty_ecs::World;

use focus::MoveRepeat;
pub use focus::{find_selectable, NavAction};
pub use raycast::raycast_pointer;

/// Metres behind a wall a world canvas stays clickable: a sign flush on it.
pub const WALL_TOLERANCE: f32 = 0.05;
/// How far a pointer ray looks for a wall, metres.
pub const WALL_RANGE: f32 = 1000.0;

/// Ray (origin, unit direction) → how far along it a world canvas may be hit, metres.
pub type Walls<'a> = &'a dyn Fn(Vec3, Vec3) -> f32;
pub use tree::{is_interactable, is_under, nearest};

/// Pixels a held pointer moves before a drag begins (Unity's default).
pub const DRAG_THRESHOLD: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UiHook {
    PointerEnter,
    PointerExit,
    PointerDown,
    PointerUp,
    PointerClick,
    BeginDrag,
    Drag,
    EndDrag,
    Scroll,
    Select,
    Deselect,
    Submit,
    Cancel,
    Move,
}

// Order is load-bearing: `as usize` indexes `key` and the event system's buttons.
rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PointerButton {
        Left = "Left",
        Right = "Right",
        Middle = "Middle",
    }
}

impl PointerButton {
    pub fn key(self) -> &'static str {
        ["MOUSE0", "MOUSE1", "MOUSE2"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    /// The button (`Left` for enter / exit / scroll).
    pub button: PointerButton,
    pub position: Vec2,
    pub delta: Vec2,
    pub target: Option<u32>,
    pub canvas_position: Option<Vec2>,
    pub canvas_delta: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Delivery {
    pub entity: u32,
    pub hook: UiHook,
    pub event: Option<PointerEvent>,
}

pub struct Frame<'a> {
    pub world: &'a World,
    pub layout: &'a UiLayout,
    pub input: &'a InputState,
    pub view: &'a UiView,
    pub walls: Walls<'a>,
    /// Whether entity `id` has a live script defining `hook`.
    pub handles: &'a dyn Fn(u32, UiHook) -> bool,
    /// This tick's unscaled seconds — the held Move's repeat clock.
    pub dt: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct ButtonState {
    held: bool,
    over_ui: bool,
    press_position: Vec2,
    press: Option<u32>,
    drag: Option<u32>,
    dragging: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EventSystem {
    selected: Option<u32>,
    announced: Option<u32>,
    hover: Vec<u32>,
    pointer: Option<Vec2>,
    last_ui: Option<UiPointer>,
    buttons: [ButtonState; 3],
    repeat: MoveRepeat,
    fades: BTreeMap<u32, transition::Fade>,
}

impl EventSystem {
    pub fn process(&mut self, frame: &Frame) -> Vec<Delivery> {
        let mut out = Vec::new();
        self.refresh_selection(frame, &mut out);
        self.navigate(frame, &mut out);
        self.process_pointer(frame, &mut out);
        out
    }

    pub fn selected(&self) -> Option<u32> {
        self.selected
    }

    pub fn set_selected(&mut self, id: Option<u32>) {
        self.selected = id;
    }

    pub fn retain_entities(&mut self, keep: impl Fn(u32) -> bool) {
        let kept = |id: Option<u32>| id.filter(|&id| keep(id));
        self.selected = kept(self.selected);
        self.announced = kept(self.announced);
        self.hover.retain(|&id| keep(id));
        for button in &mut self.buttons {
            button.press = kept(button.press);
            button.drag = kept(button.drag);
        }
        self.fades.retain(|&id, _| keep(id));
    }

    pub fn is_pointer_over_ui(&self) -> bool {
        !self.hover.is_empty()
    }

    pub fn is_pointer_consumed(&self) -> bool {
        self.is_pointer_over_ui() || self.buttons.iter().any(|b| b.held && b.over_ui)
    }

    pub fn state_of(&self, world: &World, id: u32) -> SelectionState {
        let inside = self.hover.contains(&id);
        let left = &self.buttons[0];
        if !is_interactable(world, id) {
            SelectionState::Disabled
        } else if left.held && left.press == Some(id) && inside {
            SelectionState::Pressed
        } else if self.selected == Some(id) {
            SelectionState::Selected
        } else if inside {
            SelectionState::Highlighted
        } else {
            SelectionState::Normal
        }
    }
}

/// Whether `id` may start an interaction: anything but a non-interactable Selectable.
fn accepts(world: &World, id: u32) -> bool {
    !world.has_selectable(id) || is_interactable(world, id)
}

#[cfg(test)]
mod fixture;
#[cfg(test)]
#[path = "pointer_tests.rs"]
mod pointer_tests;
#[cfg(test)]
#[path = "transition_tests.rs"]
mod transition_tests;
#[cfg(test)]
#[path = "world_tests.rs"]
mod world_tests;
