//! src/ui/events/ — the UI event system: pointer, focus and Selectable states (#420).
//!
//! Unity's `EventSystem` + `StandaloneInputModule` + `GraphicRaycaster`, run in the
//! sim on the CPU so a headless run clicks exactly what a window would. Once per
//! tick, at the head of the script phase, [`EventSystem::process`] reads this
//! tick's input and the settled layout and returns the script callbacks to fire,
//! in order — the scripting layer dispatches them directly (no event bus):
//!
//! 1. **Selection** a script changed since last tick is announced (`OnDeselect`,
//!    `OnSelect`), and a selection that went inactive or was destroyed is dropped.
//! 2. **Keyboard navigation** (`focus`): arrows move focus, Tab / Shift+Tab cycle
//!    it, Enter submits (`OnSubmit`), Escape cancels (`OnCancel`).
//! 3. **Pointer** (`pointer`): the top-most raycast target under the mouse
//!    ([`raycast()`]) drives `OnPointerEnter` / `OnPointerExit`; each mouse button's
//!    edges drive `OnPointerDown` / `OnPointerUp` / `OnPointerClick` and the drag
//!    callbacks; the wheel drives `OnScroll`.
//!
//! Events **bubble**: each goes to the nearest ancestor-or-self of the hit entity
//! that handles it. A `Selectable` that is not interactable (its own flag or a
//! `CanvasGroup` above it) cannot *start* an interaction — press, drag, scroll,
//! submit and cancel stop at it without firing. The visual state of every
//! Selectable ([`EventSystem::state_of`]) and its transition (`transition`) are
//! derived from the same state, in `LateUpdate`.
//!
//! **Pointer coordinates** are UI screen pixels: bottom-left origin, y-up, the frame
//! `UI.GetRect`'s `screen` box is in (Unity's `PointerEventData.position`). While the
//! cursor is locked (mouse-look) the pointer is off the UI, as in Unity.

mod focus;
mod pointer;
pub mod raycast;
mod transition;
mod tree;

use std::collections::BTreeMap;

use glam::Vec2;

use crate::components::SelectionState;
use crate::core::input::InputState;
use crate::ecs::World;
use crate::ui::UiLayout;

pub use focus::{nav_actions, NavAction};
pub use raycast::raycast;
pub use tree::{is_interactable, is_under, is_visible};

/// How far (screen pixels) a held pointer moves before a drag begins — Unity's
/// `EventSystem.pixelDragThreshold` default.
pub const DRAG_THRESHOLD: f32 = 10.0;

/// A UI script callback. The scripting layer maps each to its callback name.
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
}

/// A mouse button the pointer events track (Unity's `InputButton`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

impl PointerButton {
    /// Every tracked button, in processing order.
    pub const ALL: [PointerButton; 3] = [Self::Left, Self::Right, Self::Middle];

    /// The input key the button is (`Input`'s `"MOUSE0"` …).
    pub fn key(self) -> &'static str {
        ["MOUSE0", "MOUSE1", "MOUSE2"][self as usize]
    }

    /// The name scripts see in the event table.
    pub fn name(self) -> &'static str {
        ["Left", "Right", "Middle"][self as usize]
    }
}

/// What a pointer callback is told (Unity's `PointerEventData`, trimmed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    /// The button (`Left` for enter / exit / scroll).
    pub button: PointerButton,
    /// The pointer, UI screen pixels (bottom-left, y-up); `(-1, -1)` while locked.
    pub position: Vec2,
    /// Pointer motion since last tick in pixels, or the wheel for `OnScroll`.
    pub delta: Vec2,
    /// The entity the raycast hit this tick (the callback may be an ancestor of it).
    pub target: Option<u32>,
}

/// One callback to fire: `hook` on `entity`'s scripts, with `event` for the pointer
/// family (`None` for select / deselect / submit / cancel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Delivery {
    pub entity: u32,
    pub hook: UiHook,
    pub event: Option<PointerEvent>,
}

/// What one [`EventSystem::process`] reads. `handles(id, hook)` answers whether
/// entity `id` has a live script defining `hook` — the bubbling test.
pub struct Frame<'a> {
    pub world: &'a World,
    pub layout: &'a UiLayout,
    pub input: &'a InputState,
    /// The screen size in pixels the layout was computed for.
    pub screen: Vec2,
    pub handles: &'a dyn Fn(u32, UiHook) -> bool,
}

/// One mouse button's press in flight.
#[derive(Clone, Copy, Debug, Default)]
struct ButtonState {
    held: bool,
    /// The press began over a raycast target.
    over_ui: bool,
    press_position: Vec2,
    /// Receives up / click (and was sent down).
    press: Option<u32>,
    /// Receives the drag callbacks.
    drag: Option<u32>,
    dragging: bool,
}

/// The event system's state (a resource, shared with the `UI` namespace).
#[derive(Clone, Debug, Default)]
pub struct EventSystem {
    /// The focused entity, as scripts and navigation last set it.
    selected: Option<u32>,
    /// The selection the last `OnSelect` announced.
    announced: Option<u32>,
    /// The hit entity and its ancestors — everything the pointer is inside.
    hover: Vec<u32>,
    /// Last tick's pointer position.
    pointer: Option<Vec2>,
    buttons: [ButtonState; 3],
    fades: BTreeMap<u32, transition::Fade>,
}

impl EventSystem {
    /// Run one tick of UI input; returns the callbacks to fire, in order.
    pub fn process(&mut self, frame: &Frame) -> Vec<Delivery> {
        let mut out = Vec::new();
        self.refresh_selection(frame, &mut out);
        self.navigate(frame, &mut out);
        self.process_pointer(frame, &mut out);
        out
    }

    /// The focused entity.
    pub fn selected(&self) -> Option<u32> {
        self.selected
    }

    /// Focus `id` (or nothing). `OnDeselect` / `OnSelect` fire at the head of the next
    /// tick's script phase.
    pub fn set_selected(&mut self, id: Option<u32>) {
        self.selected = id;
    }

    /// Whether the pointer is over a raycast target this tick (Unity's
    /// `EventSystem.IsPointerOverGameObject`).
    pub fn is_pointer_over_ui(&self) -> bool {
        !self.hover.is_empty()
    }

    /// Whether gameplay should leave the pointer alone this tick: it is over the UI,
    /// or a button press that began over the UI is still held (dragging a slider off
    /// its edge must not fire the weapon).
    pub fn is_pointer_consumed(&self) -> bool {
        self.is_pointer_over_ui() || self.buttons.iter().any(|b| b.held && b.over_ui)
    }

    /// The visual state of Selectable `id`: `Disabled` when not interactable, else
    /// `Pressed` while the left button pressed it and the pointer is still inside,
    /// else `Selected` when focused, else `Highlighted` while hovered, else `Normal`.
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
#[path = "focus_tests.rs"]
mod focus_tests;
#[cfg(test)]
#[path = "pointer_tests.rs"]
mod pointer_tests;
#[cfg(test)]
#[path = "transition_tests.rs"]
mod transition_tests;
