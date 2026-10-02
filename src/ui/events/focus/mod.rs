//! src/ui/events/focus.rs — keyboard focus: selection and navigation (#420).
//!
//! The focused ("selected") entity is what the keyboard and gamepad drive. Input
//! becomes **logical actions** first (`actions`: [`nav_actions`] and the held
//! Move's repeat, #672) — Move, Next / Previous, Submit, Cancel — and only those
//! drive focus.
//!
//! - **Move** follows the focused Selectable's `navigation`: `Explicit` takes the
//!   `select_on_*` entity, `Automatic` the best-scoring candidate in that direction
//!   (Unity's `Selectable.FindSelectable`: the candidate centres in front of the
//!   focused rect's edge, scored `dot / distance²`), `None` stays put.
//! - **Next / Previous** (Tab / Shift+Tab) cycle the candidates in draw order,
//!   starting from the first (last) when nothing is focused.
//! - A focused entity whose scripts define `OnMove` **takes the Move itself**
//!   (Unity's `IMoveHandler` overriding `Selectable.OnMove`): it gets
//!   `OnMove(id, event)` and focus stays put — a slider turns Left / Right into
//!   value steps, an input field into caret moves. It navigates on its own with
//!   `UI.FindSelectable` (the same [`find_selectable`] rule).
//! - **Submit / Cancel** (Enter or pad `A` / Escape or pad `B`) fire `OnSubmit` / `OnCancel` on the
//!   focused entity itself — they do not bubble, as in Unity.
//!
//! A *candidate* is a visible, interactable Selectable whose navigation is not
//! `None`. Every change of focus fires `OnDeselect` on the old entity, then
//! `OnSelect` on the new one.

mod actions;

use glam::Vec2;

use super::pointer::emit;
use super::tree::is_visible;
use super::{
    accepts, is_interactable, Delivery, EventSystem, Frame, PointerButton, PointerEvent, UiHook,
};
use crate::components::NavigationMode;
use crate::ecs::World;
use crate::ui::UiLayout;

pub use actions::{held_direction, nav_actions, MoveRepeat, NavAction};

/// Unit directions in `select_on` order, y-up.
pub const DIRECTIONS: [Vec2; 4] = [Vec2::Y, Vec2::NEG_Y, Vec2::NEG_X, Vec2::X];

impl EventSystem {
    /// Drop a focus that went inactive or was destroyed, and announce a focus a
    /// script changed since last tick.
    pub(super) fn refresh_selection(&mut self, f: &Frame, out: &mut Vec<Delivery>) {
        let live = self.selected.filter(|&s| is_visible(f.world, s));
        self.select(f, live, out);
    }

    /// Move focus to `new`, firing `OnDeselect` / `OnSelect` when it changes.
    pub(super) fn select(&mut self, f: &Frame, new: Option<u32>, out: &mut Vec<Delivery>) {
        self.selected = new;
        if new == self.announced {
            return;
        }
        if let Some(old) = self.announced {
            emit(f, out, old, UiHook::Deselect, None);
        }
        if let Some(id) = new {
            emit(f, out, id, UiHook::Select, None);
        }
        self.announced = new;
    }

    /// The navigation phase of [`EventSystem::process`]: this tick's edge actions,
    /// then a held Move's repeat.
    pub(super) fn navigate(&mut self, f: &Frame, out: &mut Vec<Delivery>) {
        let mut actions = nav_actions(f.input);
        let pressed = actions.iter().any(|a| matches!(a, NavAction::Move(_)));
        let held = held_direction(f.input);
        actions.extend(self.repeat.tick(held, pressed, f.dt));
        for action in actions {
            match action {
                NavAction::Move(dir) => {
                    let Some(s) = self.selected else { continue };
                    if (f.handles)(s, UiHook::Move) {
                        if accepts(f.world, s) {
                            emit(f, out, s, UiHook::Move, Some(move_event(s, dir)));
                        }
                    } else if let Some(to) = find_selectable(f.world, f.layout, s, dir) {
                        self.select(f, Some(to), out);
                    }
                }
                NavAction::Next | NavAction::Previous => {
                    let to = cycle(f, self.selected, action == NavAction::Next);
                    if to.is_some() {
                        self.select(f, to, out);
                    }
                }
                NavAction::Submit | NavAction::Cancel => {
                    let hook = match action {
                        NavAction::Submit => UiHook::Submit,
                        _ => UiHook::Cancel,
                    };
                    if let Some(s) = self.selected.filter(|&s| accepts(f.world, s)) {
                        emit(f, out, s, hook, None);
                    }
                }
            }
        }
    }
}

/// The `OnMove` event: the direction as a unit `delta` (Unity's `AxisEventData`).
fn move_event(id: u32, dir: usize) -> PointerEvent {
    PointerEvent {
        button: PointerButton::Left,
        position: Vec2::splat(-1.0),
        delta: DIRECTIONS[dir],
        target: Some(id),
        canvas_position: None,
        canvas_delta: Vec2::ZERO,
    }
}

/// Visible, interactable, navigable Selectables, in draw order.
fn candidates(world: &World, layout: &UiLayout) -> Vec<u32> {
    layout
        .iter()
        .map(|(id, _)| id)
        .filter(|&id| {
            world
                .selectable(id)
                .is_some_and(|s| s.navigation != NavigationMode::None)
                && is_visible(world, id)
                && is_interactable(world, id)
        })
        .collect()
}

/// Where a Move in `dir` (`select_on` order) from `from` lands, per `from`'s
/// navigation mode — Unity's `Selectable.FindSelectableOn*`.
pub fn find_selectable(world: &World, layout: &UiLayout, from: u32, dir: usize) -> Option<u32> {
    let mode = world.selectable(from)?.navigation;
    match mode {
        NavigationMode::None => None,
        NavigationMode::Explicit => {
            let to = world.selectable(from)?.select_on[dir]?;
            is_visible(world, to).then_some(to)
        }
        NavigationMode::Automatic => by_geometry(world, layout, from, DIRECTIONS[dir]),
    }
}

/// Unity's automatic navigation: from the focused rect's edge in `dir`, the
/// candidate centre maximizing `dot(dir, v) / |v|²` among those in front of it.
fn by_geometry(world: &World, layout: &UiLayout, from: u32, dir: Vec2) -> Option<u32> {
    let (lo, hi) = layout.get(from)?.screen_bounds();
    let origin = (lo + hi) * 0.5 + dir * (hi - lo) * 0.5;
    let mut best: Option<(f32, u32)> = None;
    for id in candidates(world, layout)
        .into_iter()
        .filter(|&id| id != from)
    {
        let Some(rect) = layout.get(id) else {
            continue;
        };
        let (clo, chi) = rect.screen_bounds();
        let v = (clo + chi) * 0.5 - origin;
        let dot = dir.dot(v);
        if dot <= 0.0 {
            continue;
        }
        let score = dot / v.length_squared();
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, id));
        }
    }
    best.map(|(_, id)| id)
}

/// The next (or previous) candidate after `from` in draw order, wrapping.
fn cycle(f: &Frame, from: Option<u32>, forward: bool) -> Option<u32> {
    let all = candidates(f.world, f.layout);
    let n = all.len();
    if n == 0 {
        return None;
    }
    let at = from.and_then(|s| all.iter().position(|&c| c == s));
    let i = match (at, forward) {
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
        (None, true) => 0,
        (None, false) => n - 1,
    };
    Some(all[i])
}

#[cfg(test)]
mod pad_tests;
#[cfg(test)]
mod tests;
